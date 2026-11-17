<script setup lang="ts">
import { computed, onActivated, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import LcdDisplay from '../components/LcdDisplay.vue'

interface FoundDevice { output: string; deviceFound: boolean; downloadMode: boolean; deviceName: string | null }
interface BackupOption { key: string; firmwareId: string; firmwareName: string; version: string }
interface ScannedFirmware {
  id: string
  name: string
  version: string
  currentVersion?: string | null
}

const devices = ref<ScannedFirmware[]>([])
const detected = ref<FoundDevice | null>(null)
const selectedBackupTarget = ref('')
const busyAction = ref('')
const terminalLines = ref<string[]>([])
const lastError = ref('')
const infoFirmware = ref('')
const infoModel = ref('')
const backupOptions = computed<BackupOption[]>(() => devices.value.map((firmware) => ({
  key: `${firmware.id}::${firmware.version}`,
  firmwareId: firmware.id,
  firmwareName: firmware.name,
  version: firmware.version,
})))
const matchingFirmware = computed(() => {
  const raw = detected.value?.deviceName ?? ''
  if (!raw || raw.toLowerCase() === 'storagedevice') return null
  const normalized = raw.toLocaleLowerCase().replace(/\s*\([^)]*\).*$/, '').replace(/\s+(fm-1|smk-37).*$/, '').trim()
  return devices.value.filter((firmware) => normalized.includes(firmware.name.toLocaleLowerCase()) || firmware.name.toLocaleLowerCase().includes(normalized))
    .sort((a, b) => b.name.length - a.name.length || Number(b.version === b.currentVersion) - Number(a.version === a.currentVersion))[0] ?? null
})
const displayTitle = computed(() => {
  if (!detected.value?.deviceFound) return 'NO UNIT'
  if (detected.value.downloadMode) return 'FM-1 DOWNLOAD MODE'
  const raw = detected.value.deviceName ?? ''
  const board = raw.match(/SMK-37|FM-1/i)?.[0]?.toUpperCase() ?? 'FM-1'
  const firmware = infoFirmware.value || matchingFirmware.value?.name || raw.replace(/\s*\([^)]*\).*$/, '').replace(/\s+(fm-1|smk-37).*$/i, '').trim()
  return `${infoModel.value || board} ${firmware || 'CONNECTED'}`
})
const connectionStatus = computed(() => {
  if (busyAction.value === 'find') return 'SCANNING DEVICE'
  if (detected.value?.downloadMode) return 'DOWNLOAD MODE'
  if (detected.value?.deviceFound) return 'CONNECTED'
  return 'NO DEVICE'
})
const lcdMessage = computed(() => detected.value?.downloadMode ? 'WAITING FOR RESTART' : detected.value?.deviceFound ? 'DEVICE READY' : 'WAITING FOR DEVICE')
const lcdHint = computed(() => detected.value?.downloadMode ? 'RESTART DEVICE. PRESS THE REBOOT BUTTON BELOW' : 'HOLD SEQ REC (SMK-37) OR REC (FM-1) WHILE SWITCHING ON')

function appendTerminal(command: string, output: string) {
  const stamp = new Date().toLocaleTimeString()
  terminalLines.value.push(`[${stamp}] $ m-flasher ${command}`)
  if (output.trim()) terminalLines.value.push(...output.trimEnd().split(/\r?\n/))
  if (terminalLines.value.length > 300) terminalLines.value.splice(0, terminalLines.value.length - 300)
}

function matchTarget(firmware: ScannedFirmware | null) {
  if (!firmware) return false
  selectedBackupTarget.value = `${firmware.id}::${firmware.version}`
  return true
}

async function findDevice(quiet = false) {
  if (busyAction.value) return
  busyAction.value = 'find'
  if (!quiet) lastError.value = ''
  try {
    if (!isTauri()) throw new Error('M-Flasher device detection is available in the desktop app.')
    if (!devices.value.length) devices.value = await invoke<ScannedFirmware[]>('scan_installed_firmwares')
    const result = await invoke<FoundDevice>('m_flasher_find')
    const previous = detected.value
    if (previous?.deviceName !== result.deviceName || previous?.downloadMode !== result.downloadMode) {
      infoFirmware.value = ''
      infoModel.value = ''
    }
    detected.value = result
    if (!quiet || previous?.output !== result.output || previous?.deviceFound !== result.deviceFound) appendTerminal('find', result.output || 'No device found.')
    if (!matchTarget(matchingFirmware.value)) selectedBackupTarget.value = ''
    if (!result.deviceFound && !quiet) lastError.value = 'No compatible device was found.'
    if (result.deviceFound && !result.downloadMode) lastError.value = ''
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    const changed = detected.value?.output !== message || detected.value?.deviceFound !== false
    detected.value = { output: message, deviceFound: false, downloadMode: false, deviceName: null }
    if (!quiet || changed) appendTerminal('find', message)
    if (!quiet || changed) lastError.value = message
  } finally {
    busyAction.value = ''
  }
}

async function runCommand(action: 'restart' | 'info') {
  busyAction.value = action
  lastError.value = ''
  try {
    const output = await invoke<string>('m_flasher_command', { action })
    appendTerminal(action, output)
    if (action === 'info') {
      const model = output.match(/^\s*model\s+(.+)$/im)?.[1]?.trim()
      const firmware = output.match(/^\s*firmware\s+(.+)$/im)?.[1]?.trim()
      if (model) infoModel.value = model
      if (firmware) infoFirmware.value = firmware
    }
    if (action === 'restart') {
      await new Promise((resolve) => window.setTimeout(resolve, 1200))
      busyAction.value = ''
      await findDevice()
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    lastError.value = message
    appendTerminal(action, message)
  } finally {
    busyAction.value = ''
  }
}

async function backupDevice() {
  const [firmwareId, version] = selectedBackupTarget.value.split('::')
  if (!firmwareId || !version) return
  busyAction.value = 'backup'
  lastError.value = ''
  try {
    const output = await invoke<string>('m_flasher_backup', { firmwareId, version })
    appendTerminal('backup', output)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    lastError.value = message
    appendTerminal('backup', message)
  } finally {
    busyAction.value = ''
  }
}

onActivated(() => {
  void findDevice()
})
</script>

<template>
  <section class="view-panel lcd-view">
    <div class="view-heading">
      <div><p class="eyebrow">DEVICE STATUS</p><h1>LCD Display</h1></div>
      <span class="connection-pill" :class="{ 'connection-pill--online': detected?.deviceFound }"><i /> {{ connectionStatus }}</span>
    </div>
    <div class="lcd-card">
      <LcdDisplay :title="displayTitle" :message="lcdMessage" connection="USB" :hint="lcdHint" />
      <div class="lcd-actions">
        <button class="action-button" type="button" :disabled="!!busyAction || !detected?.deviceFound" @click="runCommand('restart')">{{ busyAction === 'restart' ? 'RESTARTING…' : 'RESTART' }}</button>
        <button class="action-button" type="button" :disabled="!!busyAction || !detected?.deviceFound || !selectedBackupTarget" @click="backupDevice">{{ busyAction === 'backup' ? 'SAVING…' : 'BACKUP' }}</button>
        <button class="action-button" type="button" :disabled="!!busyAction || !detected?.deviceFound" @click="runCommand('info')">{{ busyAction === 'info' ? 'READING…' : 'INFO' }}</button>
      </div>
      <label v-if="detected?.deviceFound && (!matchingFirmware || !selectedBackupTarget)" class="backup-firmware-picker">
        FIRMWARE TO ASSOCIATE WITH THIS SAVE
        <select v-model="selectedBackupTarget" :disabled="!!busyAction">
          <option value="">Select installed firmware version</option>
          <option v-for="option in backupOptions" :key="option.key" :value="option.key">{{ option.firmwareName }} — {{ option.version }}</option>
        </select>
      </label>
      <p v-if="lastError" class="lcd-error" role="status">{{ lastError }}</p>
      <p class="lcd-caption">LIVE DEVICE DISPLAY <span>—</span> {{ detected?.deviceFound ? 'Connected through M-Flasher.' : 'Checking for a compatible device.' }}</p>
    </div>
    <section class="mflasher-terminal" aria-label="M-Flasher terminal output">
      <div class="terminal-heading"><h2>M-FLASHER TERMINAL</h2><button class="outline-button" type="button" :disabled="busyAction === 'find'" @click="findDevice()">{{ busyAction === 'find' ? 'SCANNING…' : 'FIND DEVICE' }}</button></div>
      <pre class="terminal-output" aria-live="polite">{{ terminalLines.join('\n') || 'Waiting for M-Flasher output…' }}</pre>
    </section>
  </section>
</template>

<style>
.lcd-view .connection-pill { display: inline-flex; align-items: center; gap: 8px; }
.lcd-view .connection-pill i { display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: #988b8b; }
.lcd-view .connection-pill--online i { background: #68e1be; box-shadow: 0 0 8px #68e1be88; }
.lcd-card { max-width: 980px; padding: clamp(16px, 3vw, 28px); background: #282524; border: 1px solid #48413f; }
.lcd-actions { display: flex; gap: 10px; margin-top: 15px; }
.lcd-actions .action-button { flex: 1; justify-content: center; min-height: 48px; }
.backup-firmware-picker { display: grid; gap: 6px; margin-top: 14px; color: #b49bdc; font-size: 10px; letter-spacing: .13em; }
.backup-firmware-picker select { padding: 9px 10px; color: #e3dcd2; background: #282525; border: 1px solid #514754; font: 12px 'Share Tech Mono', monospace; }
.lcd-error { margin: 12px 0 0; color: #efaa9e; font: 12px 'Share Tech Mono', monospace; }
.lcd-caption { margin: 14px 0 0; color: #b49bdc; font: 11px 'Share Tech Mono', monospace; letter-spacing: .12em; }
.lcd-caption span { color: #73dfc1; }
.mflasher-terminal { max-width: 980px; margin-top: 18px; border: 1px solid #514754; background: #211f20; }
.terminal-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 9px 12px; border-bottom: 1px solid #514754; }
.terminal-heading h2 { margin: 0; color: #b49bdc; font-size: 12px; letter-spacing: .16em; }
.terminal-output { height: 240px; overflow: auto; margin: 0; padding: 12px; color: #b9d6c9; background: #171718; font: 12px/1.55 'Share Tech Mono', monospace; white-space: pre-wrap; overflow-wrap: anywhere; }
@media (max-width: 560px) { .lcd-actions { flex-direction: column; } .terminal-output { height: 200px; } }
</style>
