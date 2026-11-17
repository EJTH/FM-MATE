<script setup lang="ts">
import { computed, onDeactivated, onActivated, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { LazyStore } from '@tauri-apps/plugin-store'
import DetailedResultCard from '../components/DetailedResultCard.vue'
import FirmwareResultCard from '../components/FirmwareResultCard.vue'
import ConfirmationDialog from '../components/ConfirmationDialog.vue'
import { loadFirmwareCatalog } from '../services/firmwareCatalog'
import type { FirmwareCatalogLicense, FirmwareRecord } from '../services/firmwareCatalog'
import type { MFlasherRequestPayload } from '../types/flasher'

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
const repositoryLicense = ref<FirmwareCatalogLicense | null | undefined>()
const search = ref('')
const deviceFilter = ref<DeviceFilter>('all')
const selectedKinds = ref<string[]>([])
const sortBy = ref<'name' | 'kind' | 'added' | 'date' | 'stars' | 'installs'>('installs')
const sortDescending = ref(true)
const loading = ref(false)
const errorMessage = ref('')
const resultView = ref<'detailed' | 'grid'>('detailed')
const selectedResult = ref<FirmwareRecord | null>(null)
const tryingId = ref('')
const firmwareKeepPrompt = ref<FirmwareRecord | null>(null)
const controller = ref<AbortController | null>(null)
const emit = defineEmits<{
  'open-settings': []
  'open-installed-firmware': [firmwareId: string]
  openFlasher: [request: MFlasherRequestPayload]
}>()
const downloadedFirmwares = ref<DownloadedFirmware[]>([])
const downloadingId = ref('')
const downloadMessage = ref('')
const downloadIsError = ref(false)
const repositoryLicenseUrl = computed(() => {
  const value = repositoryLicense.value?.url
  if (!value) return ''
  try {
    const url = new URL(value)
    return ['https:', 'http:'].includes(url.protocol) ? url.toString() : ''
  } catch {
    return ''
  }
})

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
    repositoryLicense.value = catalog.license
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

function installedVersionsFor(firmware: FirmwareRecord) {
  return [...new Set(downloadedFirmwares.value
    .filter((download) => download.id === firmware.id)
    .map((download) => download.version)
    .filter(Boolean))]
    .sort((a, b) => b.localeCompare(a, undefined, { numeric: true, sensitivity: 'base' }))
}

async function downloadFirmware(firmware: FirmwareRecord, installAfterDownload = false): Promise<boolean> {
  if (!firmware.pkg || downloadingId.value) return false
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
    if (installAfterDownload) await beginFirmwareInstall(firmware)
    return true
  } catch (error) {
    downloadIsError.value = true
    downloadMessage.value = error instanceof Error ? error.message : 'Firmware download failed.'
    return false
  } finally {
    downloadingId.value = ''
  }
}

async function beginFirmwareInstall(firmware: FirmwareRecord) {
  let automaticallyBackup = true
  try {
    const preferences = isTauri()
      ? await store.get<{ automaticallyBackupBeforeSwitching?: boolean }>('preferences')
      : JSON.parse(localStorage.getItem('fm-mate.preferences') ?? '{}') as { automaticallyBackupBeforeSwitching?: boolean }
    automaticallyBackup = preferences?.automaticallyBackupBeforeSwitching ?? true
  } catch {
    // Use the enabled default if saved settings are unavailable.
  }
  emit('openFlasher', {
    type: 'switch-firmware',
    firmwareId: firmware.id,
    firmwareName: firmware.name,
    version: firmware.version ?? 'unknown',
    automaticallyBackup,
  })
}

async function installFromResult(firmware: FirmwareRecord) {
  if (!isTauri()) {
    downloadIsError.value = true
    downloadMessage.value = 'Installing firmware is available in the FM-MATE desktop app.'
    return
  }
  if (hasDownloaded(firmware)) {
    await beginFirmwareInstall(firmware)
  } else {
    await downloadFirmware(firmware, true)
  }
}

async function tryFirmware(firmware: FirmwareRecord) {
  if (!isTauri()) {
    downloadIsError.value = true
    downloadMessage.value = 'Trying firmware in FM1-Emu is available in the desktop app.'
    return
  }
  if (tryingId.value || downloadingId.value) return
  tryingId.value = firmware.id
  downloadMessage.value = ''
  downloadIsError.value = false
  try {
    if (!hasDownloaded(firmware) && !await downloadFirmware(firmware)) return
    await invoke('try_firmware_in_emulator', {
      id: firmware.id,
      version: firmware.version ?? 'unknown',
    })
    firmwareKeepPrompt.value = firmware
  } catch (error) {
    downloadIsError.value = true
    downloadMessage.value = error instanceof Error ? error.message : 'Could not open firmware in FM1-Emu.'
  } finally {
    tryingId.value = ''
  }
}

function keepTriedFirmware() {
  if (firmwareKeepPrompt.value) downloadMessage.value = `Kept ${firmwareKeepPrompt.value.name} ${firmwareKeepPrompt.value.version ?? ''}.`.trim()
  firmwareKeepPrompt.value = null
}

async function discardTriedFirmware() {
  const firmware = firmwareKeepPrompt.value
  if (!firmware) return
  firmwareKeepPrompt.value = null
  try {
    await invoke('delete_firmware_version', { id: firmware.id, version: firmware.version ?? 'unknown' })
    downloadedFirmwares.value = downloadedFirmwares.value.filter((item) => item.id !== firmware.id || item.version !== (firmware.version ?? ''))
    if (isTauri()) {
      await store.set('downloadedFirmwares', downloadedFirmwares.value)
      await store.save()
    }
    downloadMessage.value = `Removed ${firmware.name} ${firmware.version ?? ''}.`.trim()
    downloadIsError.value = false
  } catch (error) {
    downloadIsError.value = true
    downloadMessage.value = error instanceof Error ? error.message : 'Could not remove the test firmware.'
  }
}

async function openResultInfo(firmware: FirmwareRecord) {
  await loadDownloadedFirmwares()
  const installed = downloadedFirmwares.value.some((item) => item.id === firmware.id)
  if (installed) {
    selectedResult.value = null
    emit('open-installed-firmware', firmware.id)
  } else {
    selectedResult.value = firmware
  }
}

function installResultVersion(firmware: FirmwareRecord, version: string) {
  void beginFirmwareInstall({ ...firmware, version })
}

function kindLabel(firmware: FirmwareRecord) {
  return typeof firmware.kind === 'string' && firmware.kind.trim() ? firmware.kind : 'Other'
}

const availableKinds = computed(() => {
  const kinds = [...new Set(firmwares.value.map(kindLabel))]
  const counts = new Map<string, number>()
  for (const firmware of firmwares.value) {
    const kind = kindLabel(firmware)
    counts.set(kind, (counts.get(kind) ?? 0) + 1)
  }
  return kinds.sort((a, b) => (counts.get(b) ?? 0) - (counts.get(a) ?? 0))
})
const kindCounts = computed(() => {
  const counts = new Map<string, number>()
  for (const firmware of firmwares.value) {
    const kind = kindLabel(firmware)
    counts.set(kind, (counts.get(kind) ?? 0) + 1)
  }
  return counts
})

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

</script>

<template>
  <section class="view-panel">
    <div class="view-heading">
      <div><p class="eyebrow">FIRMWARE LIBRARY</p><h1>Browse Firmwares</h1></div>
    </div>

    <div class="toolbar">
      <label class="search-field"><span aria-hidden="true">⌕</span><input v-model="search" type="search" placeholder="Search name, author, device" aria-label="Search firmware" /></label>
      <select v-model="deviceFilter" aria-label="Filter by device"><option value="all">All devices</option><option value="fm1">FM-1</option><option value="smk37">SMK-37</option></select>
      <div class="result-view-toggle" role="group" aria-label="Results layout">
        <button type="button" :aria-pressed="resultView === 'detailed'" :class="{ 'is-active': resultView === 'detailed' }" @click="resultView = 'detailed'">DETAILS</button>
        <button type="button" :aria-pressed="resultView === 'grid'" :class="{ 'is-active': resultView === 'grid' }" @click="resultView = 'grid'">GRID</button>
      </div>
      <button class="outline-button" type="button" :disabled="loading" @click="loadRepository">{{ loading ? 'LOADING…' : 'REFRESH CATALOG' }}</button>
    </div>

    <div class="browse-options">
      <fieldset class="kind-filters" :disabled="!availableKinds.length">
        <legend>FILTER BY KIND</legend>
        <div>
          <div v-for="kind in availableKinds" :key="kind" class="kind-option"><input v-model="selectedKinds" type="checkbox" :value="kind" :aria-label="`Toggle ${kind}`" /><button type="button" class="kind-name" :title="`Show only ${kind}`" @click="selectedKinds = [kind]">{{ kind }} <span class="kind-count">({{ kindCounts.get(kind) ?? 0 }})</span></button></div>
          <span v-if="!availableKinds.length" class="no-kinds">Kinds appear when a catalog is loaded.</span>
        </div>
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
        <button class="sort-direction" type="button" :aria-label="sortDescending ? 'Sort ascending' : 'Sort descending'" :title="sortDescending ? 'Sort ascending' : 'Sort descending'" :aria-pressed="sortDescending" @click="sortDescending = !sortDescending"><span>{{ sortDescending ? '↓' : '↑' }}</span></button>
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
    <div v-else-if="resultView === 'detailed'" class="firmware-list" aria-label="Firmware releases">
      <DetailedResultCard v-for="firmware in visibleFirmwares" :key="firmware.id" :firmware="firmware" :downloading="downloadingId === firmware.id" :downloaded="installedVersionsFor(firmware).length > 0" :installed-versions="installedVersionsFor(firmware)" :trying="tryingId === firmware.id" @download="downloadFirmware(firmware)" @try-firmware="tryFirmware(firmware)" @info="openResultInfo(firmware)" @install-version="installResultVersion(firmware, $event)" />
    </div>
    <div v-else class="firmware-grid" aria-label="Firmware releases">
      <FirmwareResultCard
        v-for="firmware in visibleFirmwares"
        :key="firmware.id"
        :firmware="firmware"
        :busy="downloadingId === firmware.id"
        :can-install="!downloadingId && (hasDownloaded(firmware) || !!firmware.pkg)"
        @info="openResultInfo(firmware)"
        @install="installFromResult(firmware)"
      />
    </div>
    <DetailedResultCard v-if="selectedResult" :firmware="selectedResult" overlay :downloading="downloadingId === selectedResult.id" :downloaded="installedVersionsFor(selectedResult).length > 0" :installed-versions="installedVersionsFor(selectedResult)" :trying="tryingId === selectedResult.id" @download="downloadFirmware(selectedResult)" @try-firmware="tryFirmware(selectedResult)" @info="openResultInfo(selectedResult)" @install-version="installResultVersion(selectedResult, $event)" @close="selectedResult = null" />
    <ConfirmationDialog
      v-if="firmwareKeepPrompt"
      title="KEEP TEST FIRMWARE?"
      :message="`FM1-Emu has closed. Do you wish to keep ${firmwareKeepPrompt.name} ${firmwareKeepPrompt.version ?? ''}? Deleting it also removes its saves.`"
      confirm-label="KEEP FIRMWARE"
      cancel-label="DELETE TEST FIRMWARE"
      cancel-variant="danger"
      @confirm="keepTriedFirmware"
      @secondary="discardTriedFirmware"
      @cancel="keepTriedFirmware"
    />
    <p class="catalog-attribution">
      <a v-if="repositoryLicense?.attribution && repositoryLicenseUrl" :href="repositoryLicenseUrl" target="_blank" rel="noreferrer">{{ repositoryLicense.attribution }}</a>
      <span v-else-if="repositoryLicense?.attribution">{{ repositoryLicense.attribution }}</span>
      <template v-else>Huge thanks to <a href="https://thefirmwarehub.com/" target="_blank" rel="noreferrer">thefirmwarehub.com</a> for providing the data used in the firmware browser!</template>
    </p>
  </section>
</template>

<style>
.catalog-meta { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px 18px; min-height: 24px; margin: -7px 0 8px; color: #8f8580; font: 11px 'Share Tech Mono', monospace; letter-spacing: .04em; }
.browse-options { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 20px; margin: 0 0 14px; padding: 10px 12px; background: rgb(40 37 36 / 55%); border: 1px solid #48413f; }
.kind-filters { min-width: 0; margin: 0; padding: 0; border: 0; }
.kind-filters > div { display: grid; min-width: 0; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); align-items: center; gap: 8px 0px; }
.kind-filters legend { margin: 0 0 7px; color: #9b8aa8; font-size: 10px; font-weight: 700; letter-spacing: .13em; }
.kind-filters .kind-option { width: 100%; min-width: 0; display: inline-flex; align-items: center; gap: 6px; }
.kind-filters .kind-name { padding: 0; color: #d2c9bd; background: none; border: 0; font: inherit; font-size: 13px; text-transform: capitalize; white-space: nowrap; cursor: pointer; }
.kind-filters .kind-name:hover { color: #fff; }
.kind-filters .kind-count { color: #9b8aa8; }
.kind-filters input { width: 14px; height: 14px; margin: 0; accent-color: #59d3b7; }
.kind-filters:disabled { opacity: .55; }
.no-kinds { color: #8f8580; font: 11px 'Share Tech Mono', monospace; }
.sort-controls { display: flex; flex: 0 0 auto; align-items: center; gap: 7px; }
.sort-control { display: flex; flex: 0 0 auto; align-items: center; gap: 9px; color: #b49bdc; font-size: 10px; font-weight: 700; letter-spacing: .13em; }
.sort-control select { min-width: 120px; height: 32px; padding: 0 9px; color: #e0d9cf; background: #211f1e; border: 1px solid #554b58; font: 13px 'Barlow Condensed', sans-serif; }
.sort-direction { width: 32px; height: 32px; padding: 0; color: #d9c8ed; background: #292625; border: 1px solid #665772; font-size: 18px; line-height: 1; cursor: pointer; }
.sort-direction span { position:relative; top: -3px; }
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
.firmware-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); align-items: stretch; gap: 14px; }
.result-view-toggle { display: inline-flex; flex: 0 0 auto; padding: 3px; background: #211f1e; border: 1px solid #4a4442; }
.result-view-toggle button { min-height: 32px; padding: 0 10px; color: #aaa09a; background: transparent; border: 0; font-size: 11px; font-weight: 700; letter-spacing: .08em; cursor: pointer; }
.result-view-toggle button.is-active { color: #f0eae2; background: linear-gradient(135deg, #21413e, #1b3331); box-shadow: inset 0 0 0 1px #3c766a; }
.result-view-toggle button:focus-visible { outline: 2px solid #65e1c1; outline-offset: 1px; }
.download-feedback { margin: 0 0 12px; color: #72e7c3; font: 12px 'Share Tech Mono', monospace; }
.download-feedback.is-error { color: #ed9a7b; }
@media (max-width: 600px) { .catalog-source { max-width: 100%; flex-basis: 100%; }.browse-options { grid-template-columns: minmax(0, 1fr); gap: 12px; }.sort-controls { justify-content: flex-end; }.sort-control { justify-content: space-between; } }
</style>
