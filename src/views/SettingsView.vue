<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { LazyStore } from '@tauri-apps/plugin-store'
import { DEFAULT_FIRMWARE_REPOSITORY_URL } from '../config'
import ToolDownloadCard from '../components/ToolDownloadCard.vue'

const preferences = reactive({ automaticallyBackupBeforeSwitching: true, firmwareRepositoryUrl: DEFAULT_FIRMWARE_REPOSITORY_URL })
const repositoryUrlDraft = ref('')
const settingsLoaded = ref(false)
const settingsMessage = ref('')
const settingsError = ref(false)
const preferencesStore = new LazyStore('settings.json', { autoSave: true })
const firmwareFolderDraft = ref('')
const firmwareFolderMessage = ref('')
const firmwareFolderError = ref(false)
const toolStatuses = ref<Record<string, { installed: boolean; version: string | null }>>({})
const toolBusyId = ref('')
const toolMessage = ref('')
const toolError = ref(false)
const tools = [
  { id: 'm-flasher', name: 'M-Flasher', description: 'This tool is required to flash new firmwares and perform backups.', repository: 'jvitkauskas/m-flasher' },
  { id: 'fm1-emu', name: 'FM1-Emu', description: 'This emulator is optional. It enables you to test firmwares locally.', repository: 'joakimeriksson/fm1-emu' },
]

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
        await loadTools()
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

async function loadTools() {
  const statuses = await invoke<Array<{ id: string; installed: boolean; version: string | null }>>('get_tools_status')
  toolStatuses.value = Object.fromEntries(statuses.map((tool) => [tool.id, { installed: tool.installed, version: tool.version }]))
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

async function runToolAction(id: string) {
  if (!isTauri()) return
  toolBusyId.value = id
  toolMessage.value = ''
  toolError.value = false
  try {
    const result = await invoke<{ status: 'downloaded' | 'updated' | 'up-to-date'; version: string }>('check_or_update_tool', { id })
    if (result.status === 'up-to-date') toolMessage.value = `${tools.find((tool) => tool.id === id)?.name} is up to date (${result.version}).`
    else if (result.status === 'updated') toolMessage.value = `Updated ${tools.find((tool) => tool.id === id)?.name} to ${result.version}.`
    else toolMessage.value = `Downloaded ${tools.find((tool) => tool.id === id)?.name} ${result.version}.`
    await loadTools()
  } catch (error) {
    toolError.value = true
    toolMessage.value = error instanceof Error ? error.message : 'Could not download the tool.'
  } finally {
    toolBusyId.value = ''
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
      <section class="settings-group extra-downloads">
        <h2>EXTRA DOWNLOADS</h2>
        <p class="repo-description">Optional and required utilities for working with firmware. Downloads are kept in the app's tools folder.</p>
        <ToolDownloadCard
          v-for="tool in tools"
          :key="tool.id"
          :name="tool.name"
          :description="tool.description"
          :repository="tool.repository"
          :installed="toolStatuses[tool.id]?.installed ?? false"
          :version="toolStatuses[tool.id]?.version"
          :busy="toolBusyId === tool.id"
          :disabled="!isTauri()"
          @action="runToolAction(tool.id)"
        />
        <p v-if="!isTauri()" class="settings-feedback">Tool downloads are available in the desktop app.</p>
        <p v-if="toolMessage" class="settings-feedback" :class="{ 'is-error': toolError }" role="status">{{ toolMessage }}</p>
      </section>
    </div>
  </section>
</template>
