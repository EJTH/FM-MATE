<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { LazyStore } from '@tauri-apps/plugin-store'
import AppHeader from './components/AppHeader.vue'
import WelcomeDialog from './components/WelcomeDialog.vue'
import BrowseFirmwares from './views/BrowseFirmwares.vue'
import DownloadedFirmwares from './views/DownloadedFirmwares.vue'
import BackupsSaves from './views/BackupsSaves.vue'
import SettingsView from './views/SettingsView.vue'
import MFlasherView from './views/MFlasherView.vue'
import type { MFlasherLcdState, MFlasherRequest, MFlasherRequestPayload } from './types/flasher'

type View = 'browse' | 'downloaded' | 'backups' | 'settings' | 'lcd'
interface SaveFilter { firmwareId: string; version: string }
const activeView = ref<View>('browse')
const welcomeVisible = ref(false)
const appStateLoaded = ref(false)
const saveFilter = ref<SaveFilter | null>(null)
const requestedFirmwareDetailsId = ref<string | null>(null)
const flasherRequest = ref<MFlasherRequest | null>(null)
const flasherLcdState = ref<MFlasherLcdState>({
  title: 'NO UNIT',
  message: 'WAITING FOR DEVICE',
  connection: 'USB',
  hint: 'HOLD SEQ REC (SMK-37) OR REC (FM-1) WHILE SWITCHING ON',
  progress: null,
  messageClass: '',
})
let nextFlasherRequestId = 0
const libraryStore = new LazyStore('settings.json', { autoSave: true })
const validViews: View[] = ['browse', 'downloaded', 'backups', 'settings', 'lcd']
const activeComponent = computed(() => ({
  browse: BrowseFirmwares,
  downloaded: DownloadedFirmwares,
  backups: BackupsSaves,
  settings: SettingsView,
  lcd: MFlasherView,
}[activeView.value]))

function openSaves(filter: SaveFilter) {
  saveFilter.value = filter
  activeView.value = 'backups'
}

function openInstalledFirmwareDetails(firmwareId: string) {
  requestedFirmwareDetailsId.value = firmwareId
  activeView.value = 'downloaded'
}

function clearRequestedFirmwareDetails(firmwareId: string) {
  if (requestedFirmwareDetailsId.value === firmwareId) requestedFirmwareDetailsId.value = null
}

function openFlasher(request: MFlasherRequestPayload) {
  flasherRequest.value = { ...request, requestId: ++nextFlasherRequestId } as MFlasherRequest
  activeView.value = 'lcd'
}

function clearFlasherRequest(requestId: number) {
  if (flasherRequest.value?.requestId === requestId) flasherRequest.value = null
}

onMounted(async () => {
  try {
    if (isTauri()) {
      const savedView = await libraryStore.get<string>('lastActiveView')
      if (savedView && validViews.includes(savedView as View)) activeView.value = savedView as View
      const hasLaunched = await libraryStore.get<boolean>('hasCompletedWelcome')
      welcomeVisible.value = import.meta.env.DEV || !hasLaunched
    } else {
      const savedView = localStorage.getItem('fm-mate.lastActiveView')
      if (savedView && validViews.includes(savedView as View)) activeView.value = savedView as View
      welcomeVisible.value = import.meta.env.DEV || localStorage.getItem('fm-mate.hasCompletedWelcome') !== 'true'
    }
  } catch {
    // Fall back to the default tab and show onboarding if stored app state is unavailable.
    welcomeVisible.value = true
  }
  appStateLoaded.value = true

  if (!isTauri()) return
  try {
    const installed = await invoke<unknown[]>('scan_installed_firmwares')
    await libraryStore.set('downloadedFirmwares', installed)
    await libraryStore.save()
  } catch {
    // The firmware views report their own directory scan errors when opened.
  }
})

watch(activeView, async (view) => {
  if (!appStateLoaded.value) return
  try {
    if (isTauri()) {
      await libraryStore.set('lastActiveView', view)
      await libraryStore.save()
    } else {
      localStorage.setItem('fm-mate.lastActiveView', view)
    }
  } catch {
    // Tab persistence is best effort; navigation remains available if storage fails.
  }
})

async function dismissWelcome() {
  welcomeVisible.value = false
  try {
    if (isTauri()) {
      await libraryStore.set('hasCompletedWelcome', true)
      await libraryStore.save()
    } else {
      localStorage.setItem('fm-mate.hasCompletedWelcome', 'true')
    }
  } catch {
    // Keep the current session usable if onboarding state could not be saved.
  }
}
</script>

<template>
  <main class="app-shell">
    <AppHeader :active-view="activeView" :lcd-state="flasherLcdState" @navigate="activeView = $event" />
    <WelcomeDialog v-if="welcomeVisible" @dismiss="dismissWelcome" />
    <div class="app-content">
      <KeepAlive>
        <component :is="activeComponent" :key="activeView" role="tabpanel" :initial-filter="activeView === 'backups' ? saveFilter : null" :open-firmware-id="activeView === 'downloaded' ? requestedFirmwareDetailsId : null" :pending-action="activeView === 'lcd' ? flasherRequest : null" @open-settings="activeView = 'settings'" @open-saves="openSaves" @open-flasher="openFlasher" @open-installed-firmware="openInstalledFirmwareDetails" @details-opened="clearRequestedFirmwareDetails" @action-complete="clearFlasherRequest" @lcd-state="flasherLcdState = $event" />
      </KeepAlive>
    </div>
  </main>
</template>

<style>
@import './theme.css';
html { scroll-behavior: smooth; overflow: auto; overflow-y:scroll;}
#app { min-height: 100vh; }
.app-shell { min-height: 100vh; padding-bottom: 48px; }
.app-content { width: min(100%, 1500px); margin: 0 auto; padding-inline: clamp(18px, 3vw, 44px); }
@media (max-width: 600px) { .app-content { padding-inline: 14px; } }
</style>
