<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import FirmwareThumbnail from './FirmwareThumbnail.vue'
import InstallDropdown from './InstallDropdown.vue'
import type { FirmwareRecord } from '../services/firmwareCatalog'

const props = defineProps<{
  firmware: FirmwareRecord
  repositoryUrl: string
  updateAvailable?: boolean
  overlay?: boolean
  downloading?: boolean
  downloaded?: boolean
  trying?: boolean
  installedVersions?: string[]
}>()
const emit = defineEmits<{ download: []; tryFirmware: []; info: []; installVersion: [version: string]; close: [] }>()
interface GithubReleaseNotes { title: string; body: string; url: string }
const githubRelease = ref<GithubReleaseNotes | null>(null)
let releaseController: AbortController | undefined

function githubRepositoryPath() {
  const source = props.firmware.repo?.trim() || props.firmware.release?.trim()
  if (!source) return ''
  let parts: string[]
  try {
    if (/^https?:\/\//i.test(source)) {
      const url = new URL(source)
      if (!['github.com', 'www.github.com'].includes(url.hostname.toLowerCase())) return ''
      parts = url.pathname.split('/').filter(Boolean)
    } else {
      parts = source.replace(/^github\.com\//i, '').split('/').filter(Boolean)
    }
  } catch {
    return ''
  }
  const owner = parts[0]
  const repository = parts[1]?.replace(/\.git$/i, '')
  if (!owner || !repository || !/^[\w.-]+$/.test(owner) || !/^[\w.-]+$/.test(repository)) return ''
  return `${owner}/${repository}`
}

async function loadGithubRelease() {
  releaseController?.abort()
  githubRelease.value = null
  if (!props.overlay) return
  const repository = githubRepositoryPath()
  if (!repository) return
  const controller = new AbortController()
  releaseController = controller
  try {
    const response = await fetch(`https://api.github.com/repos/${repository.split('/').map(encodeURIComponent).join('/')}/releases/latest`, {
      headers: { Accept: 'application/vnd.github+json' },
      signal: controller.signal,
    })
    if (!response.ok) return
    const release = await response.json() as { name?: unknown; tag_name?: unknown; body?: unknown; html_url?: unknown }
    if (typeof release.body !== 'string' || !release.body.trim()) return
    githubRelease.value = {
      title: typeof release.name === 'string' && release.name.trim()
        ? release.name.trim()
        : typeof release.tag_name === 'string' ? release.tag_name : 'Latest release',
      body: release.body.trim(),
      url: typeof release.html_url === 'string' && /^https:\/\/github\.com\//i.test(release.html_url) ? release.html_url : '',
    }
  } catch {
    // Release notes are optional; keep the firmware details usable if GitHub is unavailable.
  } finally {
    if (releaseController === controller) releaseController = undefined
  }
}

watch(() => props.overlay ? `${props.firmware.repo ?? ''}|${props.firmware.release ?? ''}` : '', () => void loadGithubRelease(), { immediate: true })
onBeforeUnmount(() => releaseController?.abort())

function formatSize(size: number) {
  return size >= 1024 * 1024 ? `${(size / (1024 * 1024)).toFixed(1)} MiB` : `${Math.ceil(size / 1024)} KiB`
}

function openFromCard(event: MouseEvent) {
  if ((event.target as HTMLElement).closest('button, a, input, select, [role="menu"]')) return
  emit('info')
}

function openFromKeyboard(event: KeyboardEvent) {
  if (event.target !== event.currentTarget) return
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    emit('info')
  }
}
</script>

<template>
  <div v-if="overlay" class="detailed-result-backdrop" role="presentation" @click.self="emit('close')" @keydown.esc="emit('close')">
    <article class="detailed-result-card detailed-result-card--overlay" role="dialog" aria-modal="true" :aria-label="`${firmware.name} details`">
      <header class="detailed-result-titlebar">
        <span class="detailed-result-titlebar__brand">FM-MATE</span>
        <strong>FIRMWARE DETAILS</strong>
        <span class="detailed-result-titlebar__name">{{ firmware.name }}</span>
        <button type="button" aria-label="Close firmware details" @click="emit('close')">×</button>
      </header>
      <button class="detailed-result-card__image-button" type="button" aria-label="Firmware image" @click="emit('info')"><FirmwareThumbnail :id="firmware.id" :card="firmware.card" :version="firmware.version" :repository-url="repositoryUrl" :alt="`${firmware.name} thumbnail`" /></button>
      <div class="detailed-result-card__main">
        <div class="detailed-result-card__heading">
          <div><h2>{{ firmware.name }}</h2><p>{{ firmware.author || firmware.repo || firmware.id }}</p></div>
          <div class="detailed-result-card__engagement">
            <span v-if="firmware.version" class="detailed-result-card__version">v{{ firmware.version }}</span>
            <span v-if="typeof firmware.stars === 'number'" class="detailed-result-card__stars">★ {{ firmware.stars }}</span>
            <span v-if="typeof firmware.installs === 'number'" class="detailed-result-card__installs">{{ firmware.installs }} installs</span>
          </div>
        </div>
        <p v-if="firmware.what" class="detailed-result-card__description">{{ firmware.what }}</p>
        <div class="detailed-result-card__tags">
          <span v-if="firmware.product">{{ firmware.product }}</span><span v-else-if="firmware.ident">{{ firmware.ident }}</span>
          <span>{{ firmware.kind || 'Other' }}</span><span v-if="firmware.lic">{{ firmware.lic }}</span>
          <span v-if="firmware.size">{{ formatSize(firmware.size) }}</span><span v-if="firmware.beta" class="detailed-result-card__beta">BETA</span>
        </div>
      </div>
      <div class="detailed-result-card__actions">
        <span class="detailed-result-card__updated">LAST UPDATED {{ firmware.date || '—' }}</span>
        <div class="detailed-result-card__links">
          <a v-if="firmware.release" :href="firmware.release" target="_blank" rel="noreferrer">RELEASE ↗</a>
          <a v-if="firmware.site" :href="firmware.site" target="_blank" rel="noreferrer">PROJECT ↗</a>
        </div>
        <div v-if="downloaded && !trying" class="detailed-result-card__installed-actions">
          <button class="outline-button" type="button" @click="emit('info')">INFO</button>
          <InstallDropdown :versions="installedVersions ?? []" @select="emit('installVersion', $event)" />
        </div>
        <div v-else class="detailed-result-card__bottom-actions">
          <button class="try-firmware-button" type="button" :disabled="trying || downloading" @click="emit('tryFirmware')">{{ trying ? 'RUNNING…' : 'TRY' }}</button>
          <button class="outline-button detailed-result-card__download" type="button" :disabled="!firmware.pkg || downloading || downloaded || trying" @click="emit('download')">{{ downloading ? 'DOWNLOADING…' : downloaded ? 'DOWNLOADED' : 'DOWNLOAD' }}</button>
        </div>
      </div>
      <section v-if="githubRelease" class="github-release-notes">
        <header>
          <div><p>LATEST GITHUB RELEASE</p><h3>{{ githubRelease.title }}</h3></div>
          <a v-if="githubRelease.url" :href="githubRelease.url" target="_blank" rel="noreferrer">VIEW RELEASE ↗</a>
        </header>
        <div class="github-release-notes__body">{{ githubRelease.body }}</div>
      </section>
    </article>
  </div>
  <article v-else class="detailed-result-card detailed-result-card--clickable" tabindex="0" :aria-label="`Open details for ${firmware.name}`" @click="openFromCard" @keydown="openFromKeyboard">
    <button class="detailed-result-card__image-button" type="button" :aria-label="updateAvailable ? 'Open firmware information - New version!' : 'Open firmware information'" @click="emit('info')"><FirmwareThumbnail :id="firmware.id" :card="firmware.card" :version="firmware.version" :repository-url="repositoryUrl" :alt="`${firmware.name} thumbnail`" :update-available="updateAvailable" /></button>
    <div class="detailed-result-card__main">
      <div class="detailed-result-card__heading">
        <div><h2>{{ firmware.name }}</h2><p>{{ firmware.author || firmware.repo || firmware.id }}</p></div>
        <div class="detailed-result-card__engagement">
          <span v-if="firmware.version" class="detailed-result-card__version">v{{ firmware.version }}</span>
          <span v-if="typeof firmware.stars === 'number'" class="detailed-result-card__stars">★ {{ firmware.stars }}</span>
          <span v-if="typeof firmware.installs === 'number'" class="detailed-result-card__installs">{{ firmware.installs }} installs</span>
        </div>
      </div>
      <p v-if="firmware.what" class="detailed-result-card__description">{{ firmware.what }}</p>
      <div class="detailed-result-card__tags">
        <span v-if="firmware.product">{{ firmware.product }}</span><span v-else-if="firmware.ident">{{ firmware.ident }}</span>
        <span>{{ firmware.kind || 'Other' }}</span><span v-if="firmware.lic">{{ firmware.lic }}</span>
        <span v-if="firmware.size">{{ formatSize(firmware.size) }}</span><span v-if="firmware.beta" class="detailed-result-card__beta">BETA</span>
      </div>
    </div>
    <div class="detailed-result-card__actions">
      <span class="detailed-result-card__updated">LAST UPDATED {{ firmware.date || '—' }}</span>
      <div class="detailed-result-card__links">
        <a v-if="firmware.release" :href="firmware.release" target="_blank" rel="noreferrer">RELEASE ↗</a>
        <a v-if="firmware.site" :href="firmware.site" target="_blank" rel="noreferrer">PROJECT ↗</a>
      </div>
      <div v-if="downloaded && !trying" class="detailed-result-card__installed-actions">
        <button class="outline-button" type="button" @click="emit('info')">INFO</button>
        <InstallDropdown :versions="installedVersions ?? []" @select="emit('installVersion', $event)" />
      </div>
      <div v-else class="detailed-result-card__bottom-actions">
        <button class="try-firmware-button" type="button" :disabled="trying || downloading" @click="emit('tryFirmware')">{{ trying ? 'RUNNING…' : 'TRY' }}</button>
        <button class="outline-button detailed-result-card__download" type="button" :disabled="!firmware.pkg || downloading || downloaded || trying" @click="emit('download')">{{ downloading ? 'DOWNLOADING…' : downloaded ? 'DOWNLOADED' : 'DOWNLOAD' }}</button>
      </div>
    </div>
  </article>
</template>

<style>
.detailed-result-backdrop { position: fixed; z-index: 30; inset: 0; display: grid; place-items: center; overflow: auto; padding: 24px; background: rgb(0 0 0 / 72%); }
.detailed-result-card { display: grid; grid-template-columns: 156px minmax(0, 1fr) minmax(145px, auto); align-items: stretch; gap: 18px; padding: 17px 18px; background: rgb(40 37 36 / 96%); border: 1px solid #48413f; }
.detailed-result-card--clickable { cursor: pointer; transition: border-color .15s, background .15s; }
.detailed-result-card--clickable:hover, .detailed-result-card--clickable:focus-visible { background: rgb(48 44 43 / 96%); border-color: #766382; outline: none; }
.detailed-result-card > .detailed-result-card__image-button { width: 156px; height: 156px; flex-basis: 156px; padding: 0; overflow: hidden; background: transparent; border: 0; cursor: pointer; }
.detailed-result-card__image-button .firmware-thumbnail { width: 100%; height: 100%; flex-basis: auto; border: 0; }
.detailed-result-card__main { min-width: 0; }
.detailed-result-card__heading { display: flex; align-items: start; justify-content: space-between; gap: 14px; }
.detailed-result-card__heading h2 { margin: 0; color: #e8e1d7; font-size: 20px; font-weight: 600; }
.detailed-result-card__heading p { margin: 3px 0 0; color: #b49bdc; font: 12px 'Share Tech Mono', monospace; }
.detailed-result-card__engagement { display: flex; flex: 0 0 auto; flex-wrap: wrap; justify-content: flex-end; gap: 6px; }
.detailed-result-card__engagement span { padding: 4px 8px; font: 12px 'Share Tech Mono', monospace; white-space: nowrap; }
.detailed-result-card__stars { color: #f1c66e; background: #393021; border: 1px solid #74603b; }
.detailed-result-card__installs { color: #b7c6d8; background: #29313a; border: 1px solid #4b5c6e; }
.detailed-result-card__description { max-width: 95ch; margin: 12px 0; color: #b9b0a7; font: 13px/1.55 'Share Tech Mono', monospace; }
.detailed-result-card__tags { display: flex; flex-wrap: wrap; gap: 6px; }
.detailed-result-card__tags span { padding: 3px 7px; color: #c4b9cc; background: #302d30; border: 1px solid #514754; font: 10px 'Share Tech Mono', monospace; }
.detailed-result-card__tags .detailed-result-card__beta { color: #f1b36d; border-color: #795b38; }
.detailed-result-card__actions { display: flex; flex-direction: column; align-items: flex-end; gap: 8px; }
.detailed-result-card__updated { color: #a49b97; font: 10px 'Share Tech Mono', monospace; letter-spacing: .06em; white-space: nowrap; }
.detailed-result-card__version { flex: 0 0 auto; padding: 3px 7px; color: #77dfc0; background: #1f3531; border: 1px solid #3c766a; font: 12px 'Share Tech Mono', monospace; }
.detailed-result-card__links { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 9px 12px; padding-top: 4px; }
.detailed-result-card__links a { color: #bda5df; font-size: 11px; font-weight: 700; letter-spacing: .08em; text-decoration: none; white-space: nowrap; }
.detailed-result-card__links a:hover { color: #e1cff8; }
.detailed-result-card__download { margin-top: auto; }
.detailed-result-card__download:disabled { opacity: .55; cursor: not-allowed; }
.detailed-result-card__bottom-actions { display: flex; gap: 8px; margin-top: auto; }
.detailed-result-card__bottom-actions > button { min-height: 38px; padding: 7px 12px; white-space: nowrap; }
.detailed-result-card__bottom-actions .detailed-result-card__download { margin-top: 0; }
.detailed-result-card__bottom-actions .detailed-result-card__download { color: #c7e7db; background: #203633; border-color: #3b7e72; }
.detailed-result-card__bottom-actions .detailed-result-card__download:hover:not(:disabled) { color: #fff; background: #203633; border-color: #65c8b3; }
.detailed-result-card__installed-actions { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; width: 100%; margin-top: auto; }
.detailed-result-card__installed-actions > .outline-button { min-height: 38px; padding: 7px 12px; }
.detailed-result-card__installed-actions .install-dropdown { min-width: 0; }
.detailed-result-card__installed-actions .install-button { min-height: 38px; }
.try-firmware-button { color: #c2b3dc; background: #292625; border: 1px solid #665772; font: 700 11px 'Share Tech Mono', monospace; cursor: pointer; }
.try-firmware-button:hover:not(:disabled) { color: #e4d8f4; background: #332e35; border-color: #9b83b7; }
.try-firmware-button:disabled { opacity: .55; cursor: wait; }
.detailed-result-card--overlay { width: min(920px, 100%); }
.detailed-result-titlebar { grid-column: 1 / -1; display: flex; min-width: 0; min-height: 43px; align-items: center; gap: 12px; margin: -17px -18px 0; padding: 6px 9px 6px 13px; background: linear-gradient(90deg, #211f1e, #302b30); border-bottom: 1px solid #514754; }
.detailed-result-titlebar__brand { color: #72e7c3; font: italic 800 13px Arial, sans-serif; letter-spacing: -.04em; }
.detailed-result-titlebar strong { color: #c9b8df; font-size: 10px; letter-spacing: .13em; white-space: nowrap; }
.detailed-result-titlebar__name { min-width: 0; flex: 1; overflow: hidden; color: #aaa09a; font: 11px 'Share Tech Mono', monospace; text-align: right; text-overflow: ellipsis; white-space: nowrap; }
.detailed-result-titlebar button { flex: 0 0 auto; width: 29px; height: 29px; color: #d5c8d7; background: #382e35; border: 1px solid #665364; font-size: 20px; line-height: 1; cursor: pointer; }
.detailed-result-titlebar button:hover { color: #fff; background: #553b43; }
.github-release-notes { grid-column: 1 / -1; min-width: 0; padding-top: 14px; border-top: 1px solid #514754; }
.github-release-notes > header { display: flex; align-items: start; justify-content: space-between; gap: 12px; margin-bottom: 9px; }
.github-release-notes > header p { margin: 0 0 3px; color: #b49bdc; font-size: 10px; font-weight: 700; letter-spacing: .14em; }
.github-release-notes h3 { margin: 0; color: #e6dfd5; font-size: 16px; }
.github-release-notes > header a { flex: 0 0 auto; color: #bda5df; font-size: 10px; font-weight: 700; text-decoration: none; }
.github-release-notes__body { max-height: 300px; overflow: auto; padding: 12px; color: #b9b0a7; background: #211f1e; border: 1px solid #48413f; font: 12px/1.55 'Share Tech Mono', monospace; white-space: pre-wrap; overflow-wrap: anywhere; }
@media (max-width: 700px) { .detailed-result-card { grid-template-columns: 112px minmax(0, 1fr); gap: 12px; }.detailed-result-card > .detailed-result-card__image-button { width: 112px; height: 112px; flex-basis: 112px; }.detailed-result-card__actions { grid-column: 2; align-items: flex-start; }.detailed-result-card__links { justify-content: flex-start; }.detailed-result-card__download { margin-top: 0; } }
@media (max-width: 520px) { .detailed-result-backdrop { padding: 12px; }.detailed-result-card--overlay { grid-template-columns: 1fr; }.detailed-result-card--overlay > .detailed-result-card__image-button { width: 100%; height: auto; aspect-ratio: 1.4; }.detailed-result-card--overlay .detailed-result-card__actions { grid-column: 1; } }
</style>
