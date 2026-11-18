<script setup lang="ts">
import { computed, onActivated, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import type { InstalledSave } from '../types/saves'
import SavesTable from '../components/SavesTable.vue'
import type { MFlasherRequestPayload } from '../types/flasher'
import { finishEmulatorSession } from '../services/emulatorSession'
import type { EmulatorSavePrompt } from '../services/emulatorSession'

interface SaveFilter {
  firmwareId: string
  version: string
}

const props = defineProps<{ initialFilter?: SaveFilter | null }>()
const emit = defineEmits<{ openFlasher: [request: MFlasherRequestPayload] }>()

const saves = ref<InstalledSave[]>([])
const loading = ref(true)
const loadError = ref(false)
const loadErrorMessage = ref('')
const actionMessage = ref('')
const emulating = ref(false)
const selectedFirmwareId = ref('')
const selectedVersion = ref('')
const sortedSaves = computed(() => [...saves.value]
  .filter((save) => !save.fileName.endsWith('.json'))
  .sort((a, b) => b.modifiedAt - a.modifiedAt))
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
  loadErrorMessage.value = ''
  try {
    saves.value = isTauri() ? await invoke<InstalledSave[]>('scan_installed_saves') : []
  } catch (error) {
    saves.value = []
    loadError.value = true
    loadErrorMessage.value = error instanceof Error ? error.message : String(error)
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

async function emulateSave(save: InstalledSave) {
  if (emulating.value) return
  actionMessage.value = ''
  emulating.value = true
  try {
    const savePrompt = await invoke<EmulatorSavePrompt>('open_save_in_emulator', {
      firmwareId: save.firmwareId,
      version: save.version,
      relativePath: save.relativePath,
    })
    
    const saved = await finishEmulatorSession(savePrompt)
    if (saved) {
      await loadSaves()
      actionMessage.value = `Emulator session saved as ${saved.fileName}.${saved.warning ? ` ${saved.warning}` : ''}`
    } else {
      actionMessage.value = 'Emulator session not saved.'
    }
  } catch (error) {
    actionMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    emulating.value = false
  }
}

async function openSaveFolder(save: InstalledSave) {
  actionMessage.value = ''
  try {
    await invoke('open_installed_save_folder', {
      firmwareId: save.firmwareId,
      version: save.version,
      relativePath: save.relativePath,
    })
  } catch (error) {
    actionMessage.value = error instanceof Error ? error.message : 'Could not open the save folder.'
  }
}

function flashSave(save: InstalledSave) {
  actionMessage.value = ''
  emit('openFlasher', { type: 'flash-save', save })
}

async function openFirmwareFolder() {
  if (!isTauri()) {
    actionMessage.value = 'Opening the firmware folder is available in the desktop app.'
    return
  }
  try {
    await invoke('open_firmware_folder')
  } catch {
    actionMessage.value = 'Could not open the firmware folder.'
  }
}

function saveDeviceBackup() {
  actionMessage.value = ''
  emit('openFlasher', { type: 'backup' })
}
</script>

<template>
  <section class="view-panel saves-view">
    <div class="view-heading">
      <div><p class="eyebrow">DEVICE ARCHIVE</p><h1>Backups</h1></div>
      <button class="outline-button" type="button" @click="openFirmwareFolder">OPEN FIRMWARE FOLDER</button>
    </div>
    <div class="device-backup-panel">
      <div class="device-backup-heading">
        <div><p class="eyebrow">M-FLASHER</p><h2>Save device backup</h2></div>
        <button class="action-button" type="button" @click="saveDeviceBackup">PERFORM BACKUP</button>
      </div>
      <p class="device-backup-note">Continue in M-Flasher to select the firmware version and confirm the backup.</p>
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
      :load-error-message="loadErrorMessage"
      :empty-message="selectedFirmwareId || selectedVersion ? 'No saves match these filters.' : 'Saves stored with downloaded firmware versions will appear here.'"
      :emulating="emulating"
      @delete="deleteSave"
      @emulate="emulateSave"
      @open-folder="openSaveFolder"
      @flash="flashSave"
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
.device-backup-panel { margin: 0 0 18px; padding: 14px; border: 1px solid #514754; background: linear-gradient(135deg, #282625, #222324); }
.device-backup-heading, .backup-target-row { display: flex; align-items: end; justify-content: space-between; gap: 14px; }
.device-backup-heading h2 { margin: 2px 0 0; color: #e4ddd4; font-size: 17px; }
.device-backup-panel .eyebrow { margin: 0; color: #b49bdc; font-size: 10px; letter-spacing: .15em; }
.device-backup-panel label { display: grid; flex: 1; gap: 5px; color: #b49bdc; font-size: 10px; letter-spacing: .13em; }
.device-backup-panel select { width: 100%; padding: 8px 10px; color: #e3dcd2; background: #282525; border: 1px solid #514754; font: 12px 'Share Tech Mono', monospace; }
.device-found { margin: 12px 0; color: #78e6c1; font: 12px 'Share Tech Mono', monospace; }
.device-backup-note { margin: 12px 0 0; color: #aa9da5; font-size: 11px; }
.device-output { margin-top: 12px; color: #b49bdc; font: 11px 'Share Tech Mono', monospace; }
.device-output pre { max-height: 130px; overflow: auto; padding: 8px; color: #d8d0c5; background: #191818; white-space: pre-wrap; }
.device-backup-panel button:disabled { opacity: .55; }
@media (max-width: 620px) { .device-backup-heading, .backup-target-row { align-items: stretch; flex-direction: column; } }
</style>
