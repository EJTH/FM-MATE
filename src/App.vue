<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { LazyStore } from '@tauri-apps/plugin-store'
import AppHeader from './components/AppHeader.vue'
import BrowseFirmwares from './views/BrowseFirmwares.vue'
import DownloadedFirmwares from './views/DownloadedFirmwares.vue'
import BackupsSaves from './views/BackupsSaves.vue'
import SettingsView from './views/SettingsView.vue'
import LcdView from './views/LcdView.vue'

type View = 'browse' | 'downloaded' | 'backups' | 'settings' | 'lcd'
interface SaveFilter { firmwareId: string; version: string }
const activeView = ref<View>('browse')
const saveFilter = ref<SaveFilter | null>(null)
const libraryStore = new LazyStore('settings.json', { autoSave: true })
const activeComponent = computed(() => ({
  browse: BrowseFirmwares,
  downloaded: DownloadedFirmwares,
  backups: BackupsSaves,
  settings: SettingsView,
  lcd: LcdView,
}[activeView.value]))

function openSaves(filter: SaveFilter) {
  saveFilter.value = filter
  activeView.value = 'backups'
}

onMounted(async () => {
  if (!isTauri()) return
  try {
    const installed = await invoke<unknown[]>('scan_installed_firmwares')
    await libraryStore.set('downloadedFirmwares', installed)
    await libraryStore.save()
  } catch {
    // The firmware views report their own directory scan errors when opened.
  }
})
</script>

<template>
  <main class="app-shell">
    <AppHeader :active-view="activeView" @navigate="activeView = $event" />
    <div class="app-content">
      <KeepAlive>
        <component :is="activeComponent" :key="activeView" role="tabpanel" :initial-filter="activeView === 'backups' ? saveFilter : null" @open-settings="activeView = 'settings'" @open-saves="openSaves" />
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
