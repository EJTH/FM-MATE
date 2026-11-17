<script setup lang="ts">
import { computed, onActivated, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import type { InstalledSave } from '../types/saves'
import type { InstalledFirmware } from '../types/firmware'
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
const installedFirmwares = ref<InstalledFirmware[]>([])
const deviceName = ref('')
const deviceOutput = ref('')
const scanBusy = ref(false)
const backupBusy = ref(false)
const backupTarget = ref('')
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
const backupOptions = computed(() => installedFirmwares.value.flatMap((firmware) => firmware.installedVersions.map((version) => ({
  key: `${firmware.id}::${version.version}`,
  firmwareId: firmware.id,
  firmwareName: firmware.name,
  version: version.version,
}))))

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
    installedFirmwares.value = isTauri() ? await invoke<InstalledFirmware[]>('scan_installed_firmwares') : []
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
    await invoke('open_firmware_folder')
  } catch {
    actionMessage.value = 'Could not open the firmware folder.'
  }
}

function guessFirmware(name: string) {
  const normalized = name.toLocaleLowerCase().replace(/\s*\([^)]*\).*$/, '').replace(/\s+(fm-1|smk-37).*$/, '').trim()
  return installedFirmwares.value
    .filter((firmware) => normalized.includes(firmware.name.toLocaleLowerCase()) || firmware.name.toLocaleLowerCase().includes(normalized))
    .sort((a, b) => b.name.length - a.name.length)[0]
}

async function findDevice() {
  actionMessage.value = ''
  scanBusy.value = true
  deviceName.value = ''
  deviceOutput.value = ''
  try {
    if (!isTauri()) throw new Error('M-Flasher device detection is available in the desktop app.')
    const found = await invoke<{ output: string; deviceFound: boolean; deviceName: string | null }>('m_flasher_find')
    deviceOutput.value = found.output
    if (!found.deviceFound || !found.deviceName) throw new Error(found.output || 'No device found.')
    deviceName.value = found.deviceName
    const firmware = guessFirmware(found.deviceName)
    if (firmware) {
      const version = firmware.installedVersions.find((item) => item.version === firmware.currentVersion) ?? firmware.installedVersions[0]
      if (version) backupTarget.value = `${firmware.id}::${version.version}`
    } else {
      backupTarget.value = ''
    }
  } catch (error) {
    actionMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    scanBusy.value = false
  }
}

async function saveDeviceBackup() {
  const [firmwareId, version] = backupTarget.value.split('::')
  if (!firmwareId || !version) return
  backupBusy.value = true
  actionMessage.value = ''
  try {
    const result = await invoke<string>('m_flasher_backup', { firmwareId, version })
    await loadSaves()
    actionMessage.value = result || 'Device backup saved.'
  } catch (error) {
    actionMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    backupBusy.value = false
  }
}
</script>

<template>
  <section class="view-panel saves-view">
    <div class="view-heading">
      <div><p class="eyebrow">DEVICE ARCHIVE</p><h1>Backups / Saves</h1></div>
      <button class="outline-button" type="button" @click="openFirmwareFolder">OPEN FIRMWARE FOLDER</button>
    </div>
    <div class="device-backup-panel">
      <div class="device-backup-heading">
        <div><p class="eyebrow">M-FLASHER</p><h2>Save device backup</h2></div>
        <button class="action-button" type="button" :disabled="scanBusy" @click="findDevice">{{ scanBusy ? 'CHECKING DEVICE…' : 'FIND DEVICE' }}</button>
      </div>
      <p v-if="deviceName" class="device-found">DEVICE FOUND: {{ deviceName }}</p>
      <div v-if="deviceName" class="backup-target-row">
        <label>STORE SAVE WITH
          <select v-model="backupTarget" :disabled="backupBusy || !backupOptions.length">
            <option value="" disabled>Select firmware and version</option>
            <option v-for="option in backupOptions" :key="option.key" :value="option.key">{{ option.firmwareName }} — {{ option.version }}</option>
          </select>
        </label>
        <button class="action-button" type="button" :disabled="backupBusy || !backupTarget" @click="saveDeviceBackup">{{ backupBusy ? 'SAVING…' : 'SAVE BACKUP' }}</button>
      </div>
      <details v-if="deviceOutput" class="device-output"><summary>M-FLASHER OUTPUT</summary><pre>{{ deviceOutput }}</pre></details>
      <p class="device-backup-note">M-Flasher backups are listed here as saves and stored in the selected firmware version’s saves folder.</p>
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
