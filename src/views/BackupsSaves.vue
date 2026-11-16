<script setup lang="ts">
import { computed, onActivated, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { openPath } from '@tauri-apps/plugin-opener'
import type { InstalledSave } from '../types/saves'
import SavesTable from '../components/SavesTable.vue'

interface SaveFilter {
  firmwareId: string
  version: string
}

const props = defineProps<{ initialFilter?: SaveFilter | null }>()

const saves = ref<InstalledSave[]>([])
const loading = ref(true)
const loadError = ref(false)
const actionMessage = ref('')
const selectedFirmwareId = ref('')
const selectedVersion = ref('')
const sortedSaves = computed(() => [...saves.value].sort((a, b) => b.modifiedAt - a.modifiedAt))
const firmwareOptions = computed(() => [...new Map(saves.value.map((save) => [save.firmwareId, save.firmwareName])).entries()]
  .map(([id, name]) => ({ id, name })).sort((a, b) => a.name.localeCompare(b.name)))
const versionOptions = computed(() => [...new Set(saves.value
  .filter((save) => !selectedFirmwareId.value || save.firmwareId === selectedFirmwareId.value)
  .map((save) => save.version))].sort((a, b) => b.localeCompare(a, undefined, { numeric: true })))
const filteredSaves = computed(() => sortedSaves.value.filter((save) =>
  (!selectedFirmwareId.value || save.firmwareId === selectedFirmwareId.value)
  && (!selectedVersion.value || save.version === selectedVersion.value),
))

watch(() => props.initialFilter, (filter) => {
  if (!filter) return
  selectedFirmwareId.value = filter.firmwareId
  selectedVersion.value = filter.version
}, { immediate: true })

async function loadSaves() {
  loading.value = true
  loadError.value = false
  try {
    saves.value = isTauri() ? await invoke<InstalledSave[]>('scan_installed_saves') : []
  } catch {
    loadError.value = true
  } finally {
    loading.value = false
  }
}

onActivated(loadSaves)

async function deleteSave(save: InstalledSave) {
  actionMessage.value = ''
  try {
    await invoke('delete_installed_save', {
      firmwareId: save.firmwareId,
      version: save.version,
      relativePath: save.relativePath,
    })
    await loadSaves()
    actionMessage.value = `Deleted ${save.fileName}.`
  } catch (error) {
    actionMessage.value = error instanceof Error ? error.message : 'Could not delete the save.'
  }
}

async function openFirmwareFolder() {
  if (!isTauri()) {
    actionMessage.value = 'Opening the firmware folder is available in the desktop app.'
    return
  }
  try {
    const folder = await invoke<string>('get_firmware_folder')
    await openPath(folder)
  } catch {
    actionMessage.value = 'Could not open the firmware folder.'
  }
}
</script>

<template>
  <section class="view-panel saves-view">
    <div class="view-heading">
      <div><p class="eyebrow">DEVICE ARCHIVE</p><h1>Backups / Saves</h1></div>
      <button class="outline-button" type="button" @click="openFirmwareFolder">OPEN FIRMWARE FOLDER</button>
    </div>
    <div class="saves-filters">
      <label>FIRMWARE
        <select v-model="selectedFirmwareId" @change="selectedVersion = ''">
          <option value="">All firmwares</option>
          <option v-for="firmware in firmwareOptions" :key="firmware.id" :value="firmware.id">{{ firmware.name }}</option>
        </select>
      </label>
      <label>VERSION
        <select v-model="selectedVersion" :disabled="!versionOptions.length">
          <option value="">All versions</option>
          <option v-for="version in versionOptions" :key="version" :value="version">{{ version }}</option>
        </select>
      </label>
    </div>
    <p v-if="actionMessage" class="saves-message" role="status">{{ actionMessage }}</p>
    <SavesTable
      :saves="filteredSaves"
      :loading="loading"
      :load-error="loadError"
      :empty-message="selectedFirmwareId || selectedVersion ? 'No saves match these filters.' : 'Saves stored with downloaded firmware versions will appear here.'"
      @delete="deleteSave"
    />
  </section>
</template>

<style>
.saves-view .view-heading { align-items: center; }
.saves-filters { display: flex; flex-wrap: wrap; gap: 12px; margin: 0 0 14px; }
.saves-filters label { display: grid; gap: 5px; color: #b49bdc; font-size: 10px; letter-spacing: .13em; }
.saves-filters select { min-width: 180px; padding: 8px 28px 8px 10px; color: #e3dcd2; background: #282525; border: 1px solid #514754; font: 12px 'Share Tech Mono', monospace; }
.saves-filters select:disabled { opacity: .5; }
.saves-message { margin: -12px 0 14px; color: #edb58d; font: 12px 'Share Tech Mono', monospace; }
</style>
