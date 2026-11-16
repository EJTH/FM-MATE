<script setup lang="ts">
import { computed, onDeactivated, onActivated, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { LazyStore } from '@tauri-apps/plugin-store'
import FirmwareThumbnail from '../components/FirmwareThumbnail.vue'
import { loadFirmwareCatalog } from '../services/firmwareCatalog'
import type { FirmwareRecord } from '../services/firmwareCatalog'

type DeviceFilter = 'all' | 'fm1' | 'smk37'
interface DownloadedFirmware {
  id: string
  name: string
  version: string
  kind: string
  fileName: string
  path: string
  downloadedAt: string
}

const store = new LazyStore('settings.json', { autoSave: true })
const firmwares = ref<FirmwareRecord[]>([])
const updated = ref('')
const repositoryUrl = ref('')
const search = ref('')
const deviceFilter = ref<DeviceFilter>('all')
const selectedKinds = ref<string[]>([])
const sortBy = ref<'name' | 'kind' | 'added' | 'date' | 'stars' | 'installs'>('installs')
const sortDescending = ref(true)
const loading = ref(false)
const errorMessage = ref('')
const controller = ref<AbortController | null>(null)
const emit = defineEmits<{ 'open-settings': [] }>()
const downloadedFirmwares = ref<DownloadedFirmware[]>([])
const downloadingId = ref('')
const downloadMessage = ref('')
const downloadIsError = ref(false)

watch(sortBy, (field) => {
  sortDescending.value = ['added', 'date', 'stars', 'installs'].includes(field)
})

async function loadRepository() {
  controller.value?.abort()
  const request = new AbortController()
  controller.value = request
  loading.value = true
  errorMessage.value = ''

  try {
    const catalog = await loadFirmwareCatalog(request.signal)

    const previousKinds = new Set(availableKinds.value)
    const previousSelection = new Set(selectedKinds.value)
    firmwares.value = catalog.firmwares
    selectedKinds.value = availableKinds.value.filter((kind) => !previousKinds.has(kind) || previousSelection.has(kind))
    updated.value = catalog.updated
    repositoryUrl.value = catalog.repositoryUrl
    if (catalog.stale) errorMessage.value = 'Could not refresh the repository; showing the cached firmware list.'
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') return
    errorMessage.value = error instanceof Error ? error.message : 'Could not load the firmware repository.'
  } finally {
    if (controller.value === request) loading.value = false
  }
}

onActivated(loadRepository)
onDeactivated(() => controller.value?.abort())

async function loadDownloadedFirmwares() {
  try {
    if (isTauri()) {
      downloadedFirmwares.value = await invoke<DownloadedFirmware[]>('scan_installed_firmwares')
      await store.set('downloadedFirmwares', downloadedFirmwares.value)
      await store.save()
    } else {
      downloadedFirmwares.value = JSON.parse(localStorage.getItem('fm-mate.downloadedFirmwares') ?? '[]') as DownloadedFirmware[]
    }
  } catch {
    downloadedFirmwares.value = []
  }
}

onActivated(loadDownloadedFirmwares)

function hasDownloaded(firmware: FirmwareRecord) {
  return downloadedFirmwares.value.some((download) => download.id === firmware.id && download.version === (firmware.version ?? ''))
}

async function downloadFirmware(firmware: FirmwareRecord) {
  if (!firmware.pkg || downloadingId.value) return
  downloadingId.value = firmware.id
  downloadMessage.value = ''
  downloadIsError.value = false
  try {
    const packageUrl = new URL(firmware.pkg, `${new URL(repositoryUrl.value).origin}/`)
    if (!['https:', 'http:'].includes(packageUrl.protocol)) throw new Error('The firmware package URL must use HTTP or HTTPS.')
    const fileName = decodeURIComponent(packageUrl.pathname.split('/').pop() ?? '')
    if (!fileName) throw new Error('The repository entry does not include a package file name.')
    const downloadedAt = new Date().toISOString()
    const thumbnailUrl = firmware.card?.trim()
      ? new URL(firmware.card, 'https://thefirmwarehub.com/').toString()
      : `https://thefirmwarehub.com/img/w0/${encodeURIComponent(firmware.id)}.webp`

    let path: string
    if (isTauri()) {
      path = await invoke<string>('download_firmware', {
        url: packageUrl.toString(),
        id: firmware.id,
        version: firmware.version || 'unknown',
        thumbnailUrl,
        firmwareInfo: { ...firmware, fileName: 'firmware.fwsc', downloadedAt },
      })
    } else {
      const response = await fetch(packageUrl)
      if (!response.ok) throw new Error(`Firmware download failed (${response.status}).`)
      const objectUrl = URL.createObjectURL(await response.blob())
      const anchor = document.createElement('a')
      anchor.href = objectUrl
      anchor.download = fileName
      anchor.click()
      URL.revokeObjectURL(objectUrl)
      path = 'Browser downloads'
    }

    const record: DownloadedFirmware = {
      id: firmware.id,
      name: firmware.name,
      version: firmware.version ?? '',
      kind: kindLabel(firmware),
      fileName: isTauri() ? 'firmware.fwsc' : fileName,
      path,
      downloadedAt,
    }
    const entries = [record, ...downloadedFirmwares.value.filter((item) => item.id !== record.id || item.version !== record.version)]
    if (isTauri()) {
      await store.set('downloadedFirmwares', entries)
      await store.save()
    } else {
      localStorage.setItem('fm-mate.downloadedFirmwares', JSON.stringify(entries))
    }
    downloadedFirmwares.value = entries
    downloadMessage.value = `${firmware.name} downloaded.`
  } catch (error) {
    downloadIsError.value = true
    downloadMessage.value = error instanceof Error ? error.message : 'Firmware download failed.'
  } finally {
    downloadingId.value = ''
  }
}

function kindLabel(firmware: FirmwareRecord) {
  return typeof firmware.kind === 'string' && firmware.kind.trim() ? firmware.kind : 'Other'
}

const availableKinds = computed(() => [...new Set(firmwares.value.map(kindLabel))].sort((a, b) => a.localeCompare(b)))

const visibleFirmwares = computed(() => {
  const query = search.value.trim().toLocaleLowerCase()
  const filtered = firmwares.value.filter((firmware) => {
    const matchesDevice = deviceFilter.value === 'all'
      || (deviceFilter.value === 'fm1' && /FM-1/i.test(`${firmware.product ?? ''} ${firmware.ident ?? ''}`))
      || (deviceFilter.value === 'smk37' && /SMK-?37/i.test(`${firmware.product ?? ''} ${firmware.ident ?? ''}`))
    const matchesKind = selectedKinds.value.includes(kindLabel(firmware))
    const matchesSearch = !query || [firmware.name, firmware.id, kindLabel(firmware), firmware.author, firmware.repo, firmware.what, firmware.product, firmware.version]
      .some((value) => typeof value === 'string' && value.toLocaleLowerCase().includes(query))
    return matchesDevice && matchesKind && matchesSearch
  })

  return filtered.sort((a, b) => {
    if (sortBy.value === 'stars' || sortBy.value === 'installs') {
      const aValue = a[sortBy.value]
      const bValue = b[sortBy.value]
      const aValid = typeof aValue === 'number' && Number.isFinite(aValue)
      const bValid = typeof bValue === 'number' && Number.isFinite(bValue)
      if (aValid && bValid && aValue !== bValue) return sortDescending.value ? bValue - aValue : aValue - bValue
      if (aValid !== bValid) return aValid ? -1 : 1
      return a.name.localeCompare(b.name)
    }

    if (sortBy.value === 'added' || sortBy.value === 'date') {
      const aDate = Date.parse(String(a[sortBy.value] ?? ''))
      const bDate = Date.parse(String(b[sortBy.value] ?? ''))
      const aValid = Number.isFinite(aDate)
      const bValid = Number.isFinite(bDate)
      if (aValid && bValid && aDate !== bDate) return sortDescending.value ? bDate - aDate : aDate - bDate
      if (aValid !== bValid) return aValid ? -1 : 1
      return a.name.localeCompare(b.name)
    }

    const aValue = sortBy.value === 'kind' ? kindLabel(a) : a.name
    const bValue = sortBy.value === 'kind' ? kindLabel(b) : b.name
    const comparison = aValue.localeCompare(bValue, undefined, { sensitivity: 'base' }) || a.name.localeCompare(b.name)
    return sortDescending.value ? -comparison : comparison
  })
})

function formatSize(size: unknown) {
  if (typeof size !== 'number' || !Number.isFinite(size) || size <= 0) return ''
  return size >= 1024 * 1024 ? `${(size / (1024 * 1024)).toFixed(1)} MiB` : `${Math.ceil(size / 1024)} KiB`
}

function starCount(firmware: FirmwareRecord) {
  if (typeof firmware.stars === 'number' && Number.isFinite(firmware.stars)) return firmware.stars
  return undefined
}
</script>

<template>
  <section class="view-panel">
    <div class="view-heading">
      <div><p class="eyebrow">FIRMWARE LIBRARY</p><h1>Browse Firmwares</h1></div>
      <span class="connection-pill"><i /> DEVICE NOT CONNECTED</span>
    </div>

    <div class="toolbar">
      <label class="search-field"><span aria-hidden="true">⌕</span><input v-model="search" type="search" placeholder="Search name, author, device" aria-label="Search firmware" /></label>
      <select v-model="deviceFilter" aria-label="Filter by device"><option value="all">All devices</option><option value="fm1">FM-1</option><option value="smk37">SMK-37</option></select>
      <button class="outline-button" type="button" :disabled="loading" @click="loadRepository">{{ loading ? 'LOADING…' : 'REFRESH CATALOG' }}</button>
    </div>

    <div class="browse-options">
      <fieldset class="kind-filters" :disabled="!availableKinds.length">
        <legend>FILTER BY KIND</legend>
        <label v-for="kind in availableKinds" :key="kind"><input v-model="selectedKinds" type="checkbox" :value="kind" />{{ kind }}</label>
        <span v-if="!availableKinds.length" class="no-kinds">Kinds appear when a catalog is loaded.</span>
      </fieldset>
      <div class="sort-controls">
        <label class="sort-control">SORT BY
          <select v-model="sortBy" aria-label="Sort firmware by">
            <option value="name">Name</option>
            <option value="kind">Kind</option>
            <option value="added">Added</option>
            <option value="date">Updated</option>
            <option value="stars">Stars</option>
            <option value="installs">Installs</option>
          </select>
        </label>
        <button class="sort-direction" type="button" :aria-label="sortDescending ? 'Sort ascending' : 'Sort descending'" :title="sortDescending ? 'Sort ascending' : 'Sort descending'" :aria-pressed="sortDescending" @click="sortDescending = !sortDescending">{{ sortDescending ? '↓' : '↑' }}</button>
      </div>
    </div>

    <div class="catalog-meta">
      <span v-if="repositoryUrl" class="catalog-source" :title="repositoryUrl">{{ repositoryUrl }}</span>
      <span v-if="updated">UPDATED {{ updated }}</span>
      <span v-if="!loading && !errorMessage && repositoryUrl">{{ visibleFirmwares.length }} FIRMWARE{{ visibleFirmwares.length === 1 ? '' : 'S' }}</span>
    </div>
    <p v-if="downloadMessage" class="download-feedback" :class="{ 'is-error': downloadIsError }" aria-live="polite">{{ downloadMessage }}</p>

    <div v-if="loading && firmwares.length === 0" class="catalog-state"><span class="empty-mark">…</span><h2>Loading firmware catalog</h2><p>Fetching the repository manifest.</p></div>
    <div v-else-if="errorMessage" class="catalog-state catalog-state--error"><span class="empty-mark">!</span><h2>Could not load catalog</h2><p>{{ errorMessage }}</p><button class="outline-button" type="button" @click="loadRepository">TRY AGAIN</button></div>
    <div v-else-if="!repositoryUrl" class="catalog-state"><span class="empty-mark">⌁</span><h2>Set a firmware repository</h2><p>Add the manifest URL in Settings to browse releases.</p><button class="outline-button" type="button" @click="emit('open-settings')">OPEN SETTINGS</button></div>
    <div v-else-if="visibleFirmwares.length === 0" class="catalog-state"><span class="empty-mark">⌕</span><h2>{{ firmwares.length ? 'No matching firmware' : 'No firmware found' }}</h2><p>{{ firmwares.length ? 'Try another search or device filter.' : 'The repository has no valid firmware entries.' }}</p></div>
    <div v-else class="firmware-list" aria-label="Firmware releases">
      <article v-for="firmware in visibleFirmwares" :key="firmware.id" class="firmware-card">
        <FirmwareThumbnail :id="firmware.id" :card="firmware.card" :alt="`${firmware.name} thumbnail`" />
        <div class="firmware-card__main">
          <div class="firmware-card__title"><div><h2>{{ firmware.name }}</h2><p>{{ firmware.author || firmware.repo || firmware.id }}</p></div><div class="firmware-engagement"><span v-if="starCount(firmware) !== undefined" class="firmware-stars">★ {{ starCount(firmware) }}</span><span v-if="typeof firmware.installs === 'number'" class="firmware-installs">{{ firmware.installs }} installs</span></div></div>
          <p v-if="firmware.what" class="firmware-description">{{ firmware.what }}</p>
          <div class="firmware-tags"><span v-if="firmware.product">{{ firmware.product }}</span><span v-else-if="firmware.ident">{{ firmware.ident }}</span><span>{{ kindLabel(firmware) }}</span><span v-if="firmware.lic">{{ firmware.lic }}</span><span v-if="firmware.size">{{ formatSize(firmware.size) }}</span><span v-if="firmware.beta" class="beta-tag">BETA</span></div>
        </div>
        <div class="firmware-card__actions">
          <span class="firmware-updated">LAST UPDATED {{ firmware.date || '—' }}</span>
          <span v-if="firmware.version" class="version-badge">v{{ firmware.version }}</span>
          <div class="firmware-card__links"><a v-if="firmware.release" :href="firmware.release" target="_blank" rel="noreferrer">RELEASE ↗</a><a v-if="firmware.site" :href="firmware.site" target="_blank" rel="noreferrer">PROJECT ↗</a></div>
          <button class="outline-button download-button" type="button" :disabled="!firmware.pkg || !!downloadingId || hasDownloaded(firmware)" @click="downloadFirmware(firmware)">{{ downloadingId === firmware.id ? 'DOWNLOADING…' : hasDownloaded(firmware) ? 'DOWNLOADED' : 'DOWNLOAD' }}</button>
        </div>
      </article>
    </div>
    <p class="catalog-attribution">Huge thanks to <a href="https://thefirmwarehub.com/" target="_blank" rel="noreferrer">thefirmwarehub.com</a> for providing the data used in the firmware browser!</p>
  </section>
</template>

<style>
.catalog-meta { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px 18px; min-height: 24px; margin: -7px 0 8px; color: #8f8580; font: 11px 'Share Tech Mono', monospace; letter-spacing: .04em; }
.browse-options { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin: 0 0 14px; padding: 10px 12px; background: rgb(40 37 36 / 55%); border: 1px solid #48413f; }
.kind-filters { display: flex; min-width: 0; flex-wrap: wrap; align-items: center; gap: 8px 18px; margin: 0; padding: 0; border: 0; }
.kind-filters legend { float: left; margin-right: 5px; color: #9b8aa8; font-size: 10px; font-weight: 700; letter-spacing: .13em; }
.kind-filters label { display: inline-flex; align-items: center; gap: 6px; color: #d2c9bd; font-size: 13px; text-transform: capitalize; }
.kind-filters input { width: 14px; height: 14px; margin: 0; accent-color: #59d3b7; }
.kind-filters:disabled { opacity: .55; }
.no-kinds { color: #8f8580; font: 11px 'Share Tech Mono', monospace; }
.sort-controls { display: flex; flex: 0 0 auto; align-items: center; gap: 7px; }
.sort-control { display: flex; flex: 0 0 auto; align-items: center; gap: 9px; color: #b49bdc; font-size: 10px; font-weight: 700; letter-spacing: .13em; }
.sort-control select { min-width: 120px; height: 32px; padding: 0 9px; color: #e0d9cf; background: #211f1e; border: 1px solid #554b58; font: 13px 'Barlow Condensed', sans-serif; }
.sort-direction { width: 32px; height: 32px; padding: 0; color: #d9c8ed; background: #292625; border: 1px solid #665772; font-size: 18px; line-height: 1; cursor: pointer; }
.sort-direction:hover { color: #fff; background: #332e35; border-color: #9b83b7; }
.sort-direction:focus-visible { outline: 2px solid #65e1c1; outline-offset: 2px; }
.catalog-source { max-width: min(72%, 700px); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.catalog-attribution { margin: 18px 0 0; color: #aaa09a; font: 12px 'Share Tech Mono', monospace; }
.catalog-attribution a { color: #bda5df; text-decoration: none; }
.catalog-attribution a:hover { color: #e1cff8; text-decoration: underline; }
.catalog-state { min-height: min(58vh, 540px); display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 25px; text-align: center; background: rgb(40 37 36 / 70%); border: 1px solid #48413f; }
.catalog-state h2 { margin: 0 0 5px; color: #e3dcd2; font-size: 21px; font-weight: 600; }
.catalog-state p { max-width: 62ch; margin: 0 0 18px; color: #a49b97; font: 13px 'Share Tech Mono', monospace; overflow-wrap: anywhere; }
.catalog-state--error .empty-mark { color: #ed9a7b; border-color: #805b4f; }
.firmware-list { display: grid; gap: 10px; }
.firmware-card { display: grid; grid-template-columns: 156px minmax(0, 1fr) minmax(145px, auto); align-items: stretch; gap: 18px; padding: 17px 18px; background: rgb(40 37 36 / 78%); border: 1px solid #48413f; }
.firmware-card__main { min-width: 0; }
.firmware-card__title { display: flex; align-items: start; justify-content: space-between; gap: 14px; }
.firmware-card__title h2 { margin: 0; color: #e8e1d7; font-size: 20px; font-weight: 600; }
.firmware-card__title p { margin: 3px 0 0; color: #b49bdc; font: 12px 'Share Tech Mono', monospace; }
.version-badge { flex: 0 0 auto; padding: 3px 7px; color: #77dfc0; background: #1f3531; border: 1px solid #3c766a; font: 12px 'Share Tech Mono', monospace; }
.firmware-description { max-width: 95ch; margin: 12px 0; color: #b9b0a7; font: 13px/1.55 'Share Tech Mono', monospace; }
.firmware-tags { display: flex; flex-wrap: wrap; gap: 6px; }
.firmware-tags span { padding: 3px 7px; color: #c4b9cc; background: #302d30; border: 1px solid #514754; font: 10px 'Share Tech Mono', monospace; }
.firmware-stars { flex: 0 0 auto; padding: 4px 8px; color: #f1c66e; background: #393021; border: 1px solid #74603b; font: 600 13px 'Share Tech Mono', monospace; white-space: nowrap; }
.firmware-engagement { display: flex; flex: 0 0 auto; flex-wrap: wrap; justify-content: flex-end; gap: 6px; }
.firmware-installs { padding: 4px 8px; color: #b7c6d8; background: #29313a; border: 1px solid #4b5c6e; font: 12px 'Share Tech Mono', monospace; white-space: nowrap; }
.firmware-tags .beta-tag { color: #f1b36d; border-color: #795b38; }
.firmware-card__actions { display: flex; flex-direction: column; align-items: flex-end; gap: 8px; }
.firmware-updated { color: #a49b97; font: 10px 'Share Tech Mono', monospace; letter-spacing: .06em; white-space: nowrap; }
.firmware-card__links { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 9px 12px; padding-top: 4px; }
.firmware-card__links a { color: #bda5df; font-size: 11px; font-weight: 700; letter-spacing: .08em; text-decoration: none; white-space: nowrap; }
.firmware-card__links a:hover { color: #e1cff8; }
.firmware-card__actions .download-button { margin-top: auto; }
.download-button:disabled { opacity: .55; cursor: not-allowed; }
.download-feedback { margin: 0 0 12px; color: #72e7c3; font: 12px 'Share Tech Mono', monospace; }
.download-feedback.is-error { color: #ed9a7b; }
@media (max-width: 700px) { .firmware-card { grid-template-columns: 112px minmax(0, 1fr); gap: 12px; }.firmware-card .firmware-thumbnail { width: 112px; height: 112px; flex-basis: 112px; }.firmware-card__actions { grid-column: 2; align-items: flex-start; }.firmware-card__links { justify-content: flex-start; }.firmware-card__actions .download-button { margin-top: 0; } }
@media (max-width: 600px) { .catalog-source { max-width: 100%; flex-basis: 100%; }.browse-options { align-items: stretch; flex-direction: column; gap: 12px; }.kind-filters legend { flex-basis: 100%; }.sort-controls { justify-content: flex-end; }.sort-control { justify-content: space-between; } }
</style>
