<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { LazyStore } from '@tauri-apps/plugin-store'
import { DEFAULT_FIRMWARE_REPOSITORY_URL } from '../config'

const preferences = reactive({ automaticallyBackupBeforeSwitching: true, firmwareRepositoryUrl: DEFAULT_FIRMWARE_REPOSITORY_URL })
const repositoryUrlDraft = ref('')
const settingsLoaded = ref(false)
const settingsMessage = ref('')
const settingsError = ref(false)
const preferencesStore = new LazyStore('settings.json', { autoSave: true })

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
      <form class="settings-group repository-form" @submit.prevent="saveRepositoryUrl">
        <h2>FIRMWARE REPOSITORY</h2>
        <label class="repo-label" for="firmware-repository-url">Firmware repository URL</label>
        <p class="repo-description">The address used to check for available firmware releases.</p>
        <div class="repo-control"><input id="firmware-repository-url" v-model="repositoryUrlDraft" type="url" :placeholder="DEFAULT_FIRMWARE_REPOSITORY_URL" :disabled="!settingsLoaded" :aria-invalid="!repositoryUrlIsValid(repositoryUrlDraft)" /><button class="outline-button" type="submit" :disabled="!settingsLoaded || !repositoryUrlIsValid(repositoryUrlDraft)">SAVE URL</button></div>
        <p class="settings-feedback" :class="{ 'is-error': settingsError }" aria-live="polite">{{ settingsMessage }}</p>
      </form>
    </div>
  </section>
</template>
