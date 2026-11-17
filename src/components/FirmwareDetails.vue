<script setup lang="ts">
import { computed } from 'vue'
import FirmwareThumbnail from './FirmwareThumbnail.vue'
import InstallDropdown from './InstallDropdown.vue'
import type { InstalledFirmware, InstalledVersion } from '../types/firmware'
import type { FirmwareRecord } from '../services/firmwareCatalog'

const props = defineProps<{
  firmware: InstalledFirmware
  update?: FirmwareRecord | null
  updateDownloading?: boolean
  updateMessage?: string
  catalogNotice?: string
}>()
const emit = defineEmits<{
  close: []
  openInEmulator: [version: InstalledVersion]
  openEditor: [version: InstalledVersion]
  deleteVersion: [version: InstalledVersion]
  openSaves: [filter: { firmwareId: string; version: string }]
  installVersion: [version: string]
  downloadUpdate: []
  openRelease: []
}>()

const releaseUrl = computed(() => {
  if (typeof props.update?.release !== 'string') return ''
  try {
    const url = new URL(props.update.release)
    return ['https:', 'http:'].includes(url.protocol) ? url.toString() : ''
  } catch {
    return ''
  }
})
const installedVersions = computed(() => props.firmware.installedVersions.map((item) => item.version))

const detailsLinks = computed(() => {
  const info = props.firmware.info
  const labels: Record<string, string> = {
    repo: 'Repository', release: 'Release', site: 'Project site', play: 'Try online',
    cheat: 'Manual', cheats: 'Manual', talk: 'Discussion', source: 'Source',
    fmhub: 'Firmware Hub', loader: 'Loader', install: 'Install guide', pkg: 'Package', mirror: 'Mirror',
  }
  return Object.keys(labels).flatMap((key) => {
    const value = info[key]
    if (typeof value !== 'string' || !value.trim()) return []
    let href = value.trim()
    if (key === 'repo' && /^[\w.-]+\/[\w.-]+$/.test(href)) {
      href = `https://github.com/${href}`
    } else if (['pkg', 'mirror'].includes(key) && !/^https?:\/\//i.test(href)) {
      href = new URL(href, 'https://thefirmwarehub.com/').toString()
    }
    try {
      const url = new URL(href)
      if (!['https:', 'http:'].includes(url.protocol)) return []
      return [{ key, label: labels[key], href: url.toString() }]
    } catch {
      return []
    }
  })
})

function editorUrl() {
  const value = props.firmware.info.editor
  if (typeof value !== 'string' || !value.trim()) return ''
  try {
    const url = new URL(value)
    return ['https:', 'http:'].includes(url.protocol) ? url.toString() : ''
  } catch {
    return ''
  }
}
</script>

<template>
  <div class="details-backdrop" @click.self="emit('close')" @keydown.esc="emit('close')">
    <section class="firmware-details" role="dialog" aria-modal="true" :aria-label="`${firmware.name} details`">
      <button class="details-close" type="button" aria-label="Close details" @click="emit('close')">×</button>
      <p class="eyebrow">FIRMWARE DETAILS</p>
      <div class="details-overview">
        <div class="details-thumbnail">
          <img v-if="firmware.thumbnailData" :src="firmware.thumbnailData" :alt="`${firmware.name} thumbnail`" />
          <FirmwareThumbnail v-else :id="firmware.id" :alt="`${firmware.name} thumbnail`" />
        </div>
        <div class="details-summary">
          <h2>{{ firmware.name }}</h2>
          <p v-if="firmware.author" class="details-author">{{ firmware.author }}</p>
          <p class="details-description">{{ firmware.description || 'No description provided.' }}</p>
          <dl><dt>Firmware ID</dt><dd>{{ firmware.id }}</dd><dt>Kind</dt><dd>{{ firmware.kind }}</dd></dl>
        </div>
      </div>
      <section v-if="update" class="details-update">
        <div><strong>NEW VERSION AVAILABLE</strong><span>v{{ update.version }}</span></div>
        <div class="details-update__actions">
          <button class="update-download-button" type="button" :disabled="updateDownloading || !update.pkg" @click="emit('downloadUpdate')">{{ updateDownloading ? 'DOWNLOADING…' : `Download new version (v${update.version})` }}</button>
          <button v-if="releaseUrl" class="release-notes-button" type="button" @click="emit('openRelease')">RELEASE NOTES ↗</button>
        </div>
      </section>
      <p v-if="catalogNotice" class="details-catalog-notice" role="status">{{ catalogNotice }}</p>
      <p v-if="updateMessage" class="details-update-message" role="status">{{ updateMessage }}</p>
      <section class="details-versions">
        <div class="details-versions__heading">
          <h3>INSTALLED VERSIONS &amp; SAVES</h3>
          <InstallDropdown :versions="installedVersions" @select="emit('installVersion', $event)" />
        </div>
        <article v-for="version in firmware.installedVersions" :key="version.version" class="details-version">
          <div class="details-version__heading"><strong>{{ version.version }}{{ firmware.currentVersion === version.version ? ' · CURRENT' : '' }}</strong><span>{{ version.saves.length }} {{ version.saves.length === 1 ? 'save' : 'saves' }}</span></div>
          <div class="details-version__actions">
            <button class="details-emulator-link" type="button" @click="emit('openInEmulator', version)">OPEN IN EMULATOR</button>
            <button v-if="editorUrl()" class="details-editor-link" type="button" @click="emit('openEditor', version)">EDITOR ↗</button>
            <button class="details-saves-link" type="button" @click="emit('openSaves', { firmwareId: firmware.id, version: version.version })">SAVES ↗</button>
            <button class="details-delete-link" type="button" @click="emit('deleteVersion', version)">DELETE</button>
          </div>
          <ul v-if="version.saves.length"><li v-for="save in version.saves" :key="save">{{ save }}</li></ul>
          <p v-else class="no-version-saves">No saves in this version.</p>
        </article>
      </section>
      <section v-if="detailsLinks.length" class="details-links"><h3>LINKS</h3><a v-for="link in detailsLinks" :key="link.key" :class="{ 'manual-link': link.key === 'cheat' || link.key === 'cheats' }" :href="link.href" target="_blank" rel="noreferrer">{{ link.key === 'cheats' ? 'Documentation' : link.label }} ↗</a></section>
    </section>
  </div>
</template>

<style>
.details-backdrop { position: fixed; z-index: 20; inset: 0; display: grid; place-items: center; padding: 20px; background: rgb(8 8 8 / 75%); }
.firmware-details { position: relative; width: min(100%, 560px); max-height: calc(100vh - 40px); overflow-y: auto; padding: 28px; color: #e3dcd2; background: #292625; border: 1px solid #665772; box-shadow: 0 18px 60px #0009; }
.firmware-details h2 { margin: 0; font-size: 28px; }.details-author { margin: 4px 0 0; color: #b49bdc; font: 12px 'Share Tech Mono', monospace; }.details-description { margin: 12px 0; color: #c4bbb1; font: 13px/1.55 'Share Tech Mono', monospace; }
.details-close { position: absolute; top: 10px; right: 12px; color: #c7bdc9; background: transparent; border: 0; font-size: 26px; cursor: pointer; }
.firmware-details dl { display: grid; grid-template-columns: 150px 1fr; gap: 8px 12px; margin: 0 0 20px; font: 12px 'Share Tech Mono', monospace; }.firmware-details dt { color: #a995bf; }.firmware-details dd { margin: 0; overflow-wrap: anywhere; }
.details-overview { display: grid; grid-template-columns: 132px minmax(0, 1fr); align-items: start; gap: 18px; margin: 10px 0 18px; }.details-summary { min-width: 0; }.details-summary dl { grid-template-columns: 90px minmax(0, 1fr); margin-bottom: 0; }.details-thumbnail { width: 132px; aspect-ratio: 1; overflow: hidden; background: #211f1e; border: 1px solid #514754; }.details-thumbnail > img { display: block; width: 100%; height: 100%; object-fit: cover; }.details-thumbnail .firmware-thumbnail { width: 100%; height: 100%; flex-basis: auto; border: 0; }
.details-update { clear: both; display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; margin: 18px 0; padding: 13px; background: #1d3029; border: 1px solid #4d9b76; }.details-update > div:first-child { display: grid; gap: 4px; }.details-update > div:first-child strong { color: #83e0a8; font-size: 10px; letter-spacing: .13em; }.details-update > div:first-child span { color: #d9f5e2; font: 16px 'Share Tech Mono', monospace; }.details-update__actions { display: flex; flex-wrap: wrap; gap: 8px; }.update-download-button, .release-notes-button { min-height: 40px; padding: 8px 12px; font: 12px 'Share Tech Mono', monospace; cursor: pointer; }.update-download-button { color: #10231a; background: #76dfa0; border: 1px solid #a1f2bd; font-weight: 700; }.update-download-button:hover { background: #9bf0b8; }.update-download-button:disabled { opacity: .6; cursor: wait; }.release-notes-button { color: #c5e8d1; background: #263a30; border: 1px solid #548669; }.release-notes-button:hover { color: #fff; border-color: #8dd5a5; }.details-catalog-notice, .details-update-message { margin: -9px 0 14px; color: #9caaa0; font: 11px 'Share Tech Mono', monospace; }
.details-versions, .details-links { margin: 18px 0; padding-top: 13px; border-top: 1px solid #514754; }.details-versions h3, .details-links h3 { margin: 0; color: #b49bdc; font-size: 11px; letter-spacing: .15em; }.details-versions__heading { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 10px; }
.details-version { padding: 9px 10px; background: #211f1e; border: 1px solid #48413f; }.details-version + .details-version { margin-top: 7px; }.details-version__heading { display: flex; justify-content: space-between; gap: 10px; color: #e3dcd2; font: 12px 'Share Tech Mono', monospace; }.details-version__heading span { color: #72e7c3; }.details-version ul { display: grid; gap: 4px; margin: 8px 0 0; padding-left: 18px; color: #b9b0a7; font: 11px 'Share Tech Mono', monospace; overflow-wrap: anywhere; }.no-version-saves { margin: 7px 0 0; color: #8f8580; font: 11px 'Share Tech Mono', monospace; }
.details-version__actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 7px; margin-top: 8px; }.details-emulator-link, .details-editor-link, .details-saves-link, .details-delete-link { padding: 4px 7px; font-size: 10px; cursor: pointer; }.details-emulator-link { color: #d2c3f0; background: #302b3b; border: 1px solid #665582; }.details-emulator-link:hover { color: #f2eaff; border-color: #9b83c8; }.details-editor-link { color: #b6e5b1; background: #26342a; border: 1px solid #4e7953; }.details-saves-link { color: #b6e5d8; background: #203633; border: 1px solid #3b7e72; }.details-delete-link { color: #efaaa3; background: #382625; border: 1px solid #80504b; }.details-delete-link:hover { color: #ffe2df; background: #4a2a28; border-color: #b56b63; }
.details-links { display: flex; flex-wrap: wrap; gap: 8px; }.details-links h3 { flex-basis: 100%; }.details-links a { padding: 5px 8px; color: #c2b3dc; background: #302d30; border: 1px solid #514754; font-size: 12px; text-decoration: none; }.details-links a:hover { color: #e4d8f4; border-color: #9b83b7; }.details-links a.manual-link { color: #b6e5b1; background: #26342a; border-color: #4e7953; }.details-links a.manual-link:hover { color: #e5ffe1; border-color: #79ac7e; }
@media (max-width: 600px) { .firmware-details { padding: 24px 18px; }.details-overview { grid-template-columns: 100px minmax(0, 1fr); gap: 12px; }.details-thumbnail { width: 100px; }.firmware-details dl { grid-template-columns: 1fr; gap: 4px; }.firmware-details dd { margin-bottom: 8px; } }
</style>
