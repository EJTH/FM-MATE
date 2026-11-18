<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { LazyStore } from '@tauri-apps/plugin-store'
import { DEFAULT_FIRMWARE_REPOSITORY_URL, THUMBNAIL_CACHE_CLEARED_EVENT, THUMBNAIL_CACHE_NAME } from '../config'
import ToolDownloadsSection from '../components/ToolDownloadsSection.vue'
import AppUpdateSection from '../components/AppUpdateSection.vue'
import type { MFlasherRequestPayload } from '../types/flasher'

const emit = defineEmits<{ openFlasher: [request: MFlasherRequestPayload] }>()

const preferences = reactive({ automaticallyBackupBeforeSwitching: true, firmwareRepositoryUrl: DEFAULT_FIRMWARE_REPOSITORY_URL })
const repositoryUrlDraft = ref('')
const settingsLoaded = ref(false)
const settingsMessage = ref('')
const settingsError = ref(false)
const preferencesStore = new LazyStore('settings.json', { autoSave: true })
const firmwareFolderDraft = ref('')
const firmwareFolderMessage = ref('')
const firmwareFolderError = ref(false)
const mFlasherInstalled = ref<boolean | null | undefined>(undefined)
const clearingCache = ref(false)
const cacheMessage = ref('')
const cacheError = ref(false)

onMounted(async () => {
  try {
    if (isTauri()) {
      const saved = await preferencesStore.get<Partial<typeof preferences>>('preferences')
      if (saved) Object.assign(preferences, saved)
    } else {
      const saved = localStorage.getItem('fm-mate.preferences')
      if (saved) Object.assign(preferences, JSON.parse(saved))
    }
    if (!preferences.firmwareRepositoryUrl.trim()) {
      preferences.firmwareRepositoryUrl = DEFAULT_FIRMWARE_REPOSITORY_URL
    }
    repositoryUrlDraft.value = preferences.firmwareRepositoryUrl
    settingsLoaded.value = true
    if (isTauri()) {
      try {
        firmwareFolderDraft.value = await invoke<string>('get_firmware_folder_path')
      } catch {
        firmwareFolderError.value = true
        firmwareFolderMessage.value = 'Could not load the firmware folder or tool status.'
      }
    }
  } catch {
    settingsError.value = true
    settingsMessage.value = 'Could not load saved settings.'
  }
})

async function savePreferences() {
  if (!settingsLoaded.value) return
  try {
    if (isTauri()) {
      await preferencesStore.set('preferences', { ...preferences })
      await preferencesStore.save()
    } else {
      localStorage.setItem('fm-mate.preferences', JSON.stringify(preferences))
    }
    settingsError.value = false
    settingsMessage.value = 'Settings saved.'
  } catch {
    settingsError.value = true
    settingsMessage.value = 'Could not save settings.'
  }
}

async function saveFirmwareFolderPath(path = firmwareFolderDraft.value) {
  if (!settingsLoaded.value || !isTauri()) return
  firmwareFolderError.value = false
  firmwareFolderMessage.value = ''
  try {
    firmwareFolderDraft.value = await invoke<string>('set_firmware_folder_path', { path })
    firmwareFolderMessage.value = 'Firmware folder saved. Existing files are not moved.'
  } catch (error) {
    firmwareFolderError.value = true
    firmwareFolderMessage.value = error instanceof Error ? error.message : 'Could not save firmware folder.'
  }
}

function repositoryUrlIsValid(value: string) {
  if (!value.trim()) return true
  try {
    const url = new URL(value)
    return url.protocol === 'https:' || url.protocol === 'http:'
  } catch {
    return false
  }
}

async function saveRepositoryUrl() {
  if (!repositoryUrlIsValid(repositoryUrlDraft.value)) {
    settingsError.value = true
    settingsMessage.value = 'Enter a valid HTTP or HTTPS repository URL.'
    return
  }
  preferences.firmwareRepositoryUrl = repositoryUrlDraft.value.trim()
  await savePreferences()
}

async function clearImageCache() {
  if (clearingCache.value) return
  clearingCache.value = true
  cacheMessage.value = ''
  cacheError.value = false
  try {
    if (isTauri()) await invoke('clear_thumbnail_cache')
    else if ('caches' in window) await caches.delete(THUMBNAIL_CACHE_NAME)
    window.dispatchEvent(new Event(THUMBNAIL_CACHE_CLEARED_EVENT))
    cacheMessage.value = 'Image cache cleared.'
  } catch (error) {
    cacheError.value = true
    cacheMessage.value = error instanceof Error ? error.message : typeof error === 'string' ? error : 'Could not clear the image cache.'
  } finally {
    clearingCache.value = false
  }
}
</script>

<template>
  <section class="view-panel settings-view">
    <div class="view-heading"><div><p class="eyebrow">PREFERENCES</p><h1>Settings</h1></div></div>
    <div class="settings-card">
      <div class="settings-group"><h2>BACKUP</h2>
        <label class="setting-row"><span><strong>Automatically backup before switching</strong><small>Create a device backup before installing a different firmware.</small></span><input v-model="preferences.automaticallyBackupBeforeSwitching" type="checkbox" :disabled="!settingsLoaded" @change="savePreferences" /></label>
      </div>
      <form class="settings-group" @submit.prevent="saveFirmwareFolderPath()">
        <h2>FIRMWARE STORAGE</h2>
        <label class="repo-label" for="firmware-folder-path">Firmware folder path</label>
        <p class="repo-description">Downloaded firmware, saves, and editors are stored here.</p>
        <div class="repo-control path-control">
          <input id="firmware-folder-path" v-model="firmwareFolderDraft" type="text" placeholder="Absolute folder path" :disabled="!settingsLoaded || !isTauri()" />
          <button class="outline-button" type="submit" :disabled="!settingsLoaded || !isTauri()">SAVE PATH</button>
          <button class="outline-button" type="button" :disabled="!settingsLoaded || !isTauri()" @click="saveFirmwareFolderPath('')">USE DEFAULT</button>
        </div>
        <p class="settings-feedback" :class="{ 'is-error': firmwareFolderError }" aria-live="polite">{{ firmwareFolderMessage }}</p>
      </form>
      <form class="settings-group repository-form" @submit.prevent="saveRepositoryUrl">
        <h2>FIRMWARE REPOSITORY</h2>
        <label class="repo-label" for="firmware-repository-url">Firmware repository URL</label>
        <p class="repo-description">The address used to check for available firmware releases.</p>
        <div class="repo-control"><input id="firmware-repository-url" v-model="repositoryUrlDraft" type="url" :placeholder="DEFAULT_FIRMWARE_REPOSITORY_URL" :disabled="!settingsLoaded" :aria-invalid="!repositoryUrlIsValid(repositoryUrlDraft)" /><button class="outline-button" type="submit" :disabled="!settingsLoaded || !repositoryUrlIsValid(repositoryUrlDraft)">SAVE URL</button></div>
        <p class="settings-feedback" :class="{ 'is-error': settingsError }" aria-live="polite">{{ settingsMessage }}</p>
      </form>
      <AppUpdateSection />
      <section class="settings-group">
        <h2>M-BOOTLOADER</h2>
        <p class="repo-description">Installing m-bootloader on your FM-1 makes it easier to recover from interrupted firmware writes. Connect your device, then use the button below to flash it. Do not disconnect or power off the device while flashing the bootloader.</p>
        <button class="action-button action-button--primary" type="button" :disabled="!isTauri() || mFlasherInstalled !== true" @click="emit('openFlasher', { type: 'flash-bootloader' })">{{ !isTauri() || mFlasherInstalled === true ? 'FLASH M-BOOTLOADER' : mFlasherInstalled === false ? 'M-flasher required, install below!' : mFlasherInstalled === null ? 'M-FLASHER STATUS UNAVAILABLE' : 'CHECKING M-FLASHER…' }}</button>
        <p v-if="!isTauri()" class="settings-feedback">Bootloader flashing is available in the desktop app.</p>
      </section>
      <ToolDownloadsSection @m-flasher-status="mFlasherInstalled = $event" />
      <section class="settings-group">
        <h2>IMAGE CACHE</h2>
        <p class="repo-description">Clear cached firmware images. Downloaded firmware and its saved thumbnails are kept.</p>
        <button class="outline-button" type="button" :disabled="clearingCache" @click="clearImageCache">{{ clearingCache ? 'CLEARING…' : 'CLEAR IMAGE CACHE' }}</button>
        <p v-if="cacheMessage" class="settings-feedback" :class="{ 'is-error': cacheError }" role="status">{{ cacheMessage }}</p>
      </section>
    </div>
  </section>
</template>
