<script setup lang="ts">
import { computed, onActivated, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import LcdDisplay from '../components/LcdDisplay.vue'
import ConfirmationDialog from '../components/ConfirmationDialog.vue'
import type { MFlasherLcdState, MFlasherRequest, MFlasherRequestPayload } from '../types/flasher'

const props = defineProps<{ pendingAction?: MFlasherRequest | null }>()
const emit = defineEmits<{ actionComplete: [requestId: number]; lcdState: [state: MFlasherLcdState]; openFlasher: [request: MFlasherRequestPayload] }>()

interface FoundDevice { output: string; deviceFound: boolean; downloadMode: boolean; deviceName: string | null }
interface BackupOption { key: string; firmwareId: string; firmwareName: string; version: string; model: string }
interface ScannedFirmware {
  id: string
  name: string
  version: string
  currentVersion?: string | null
  downloadedAt?: string
  info?: Record<string, unknown>
}

const devices = ref<ScannedFirmware[]>([])
const detected = ref<FoundDevice | null>(null)
const selectedBackupTarget = ref('')
const busyAction = ref('')
const terminalLines = ref<string[]>([])
const terminalOutput = ref<HTMLElement | null>(null)
watch(terminalLines, () => {
  if (terminalOutput.value) terminalOutput.value.scrollTop = terminalOutput.value.scrollHeight
}, { deep: true, flush: 'post' })
const lastError = ref('')
const infoFirmware = ref('')
const infoModel = ref('')
const bootloaderInfo = ref('')
const backupProgress = ref<number | null>(null)
const flashProgress = ref<number | null>(null)
const lcdStatusError = ref('')
const lcdCompletion = ref('')
const switchStage = ref<'choose' | 'flash'>('choose')
const confirmationDialog = ref<{ title: string; message: string; confirmLabel: string; cancelLabel: string; onConfirm: () => void } | null>(null)
let handledRequestId = 0
let shownRequestId = 0
const backupOptions = computed<BackupOption[]>(() => devices.value.map((firmware) => ({
  key: `${firmware.id}::${firmware.version}`,
  firmwareId: firmware.id,
  firmwareName: firmware.name,
  version: firmware.version,
  model: modelFromProduct(firmware.info?.product),
})))
const selectedBackupOption = computed(() => backupOptions.value.find((option) => option.key === selectedBackupTarget.value) ?? null)
const pendingActionReady = computed(() => {
  const request = props.pendingAction
  if (request?.type === 'backup' || (request?.type === 'switch-firmware' && request.automaticallyBackup)) {
    return !!selectedBackupTarget.value
  }
  return true
})
const matchingFirmware = computed(() => {
  const raw = detected.value?.deviceName ?? ''
  if (!raw || raw.toLowerCase() === 'storagedevice') return null
  const normalized = raw.toLocaleLowerCase().replace(/\s*\([^)]*\).*$/, '').replace(/\s+(fm-1|smk-37).*$/, '').trim()
  return devices.value.filter((firmware) => normalized.includes(firmware.name.toLocaleLowerCase()) || firmware.name.toLocaleLowerCase().includes(normalized))
    .sort((a, b) => b.name.length - a.name.length || Number(b.version === b.currentVersion) - Number(a.version === a.currentVersion))[0] ?? null
})
const displayTitle = computed(() => {
  if (!detected.value?.deviceFound) return 'NO UNIT'
  const raw = detected.value.deviceName ?? ''
  const board = raw.match(/SMK-37|FM-1/i)?.[0]?.toUpperCase() || selectedBackupOption.value?.model || ''
  const firmware = infoFirmware.value || matchingFirmware.value?.name || (detected.value.downloadMode ? selectedBackupOption.value?.firmwareName : raw.replace(/\s*\([^)]*\).*$/, '').replace(/\s+(fm-1|smk-37).*$/i, '').trim())
  const request = props.pendingAction
  if (request && request.type !== 'backup' && request.type !== 'flash-bootloader') {
    const target = request.type === 'flash-save' ? request.save.firmwareName : request.firmwareName
    return [infoModel.value || board, `${firmware || 'UNKNOWN'} -> ${target}`].filter(Boolean).join(' ')
  }
  return [infoModel.value || board, firmware || (!board ? raw : 'CONNECTED')].filter(Boolean).join(' ')
})
const connectionStatus = computed(() => {
  if (busyAction.value === 'find') return 'SCANNING DEVICE'
  if (busyAction.value === 'download-mode') return 'ENTERING DOWNLOAD MODE'
  if (detected.value?.downloadMode) return 'DOWNLOAD MODE'
  if (detected.value?.deviceFound) return 'CONNECTED'
  return 'NO DEVICE'
})
const pendingActionLabel = computed(() => {
  const request = props.pendingAction
  if (!request) return ''
  if (request.type === 'backup') return 'PERFORM BACKUP'
  if (request.type === 'flash-bootloader') return 'FLASH M-BOOTLOADER'
  if (request.type === 'switch-firmware') {
    if (request.automaticallyBackup) return 'BACKUP & FLASH'
    return switchStage.value === 'flash' ? 'FLASH FIRMWARE' : 'BACKUP BEFORE FLASH?'
  }
  if (request.type === 'flash-firmware') return 'FLASH FIRMWARE'
  return request.save.fileName.toLocaleLowerCase().endsWith('.fwsc') ? 'BACKUP' : 'WRITE BACKUP TO FLASH'
})
const pendingActionDetail = computed(() => {
  const request = props.pendingAction
  if (!request) return ''
  if (request.type === 'flash-bootloader') return 'KEEP DEVICE CONNECTED WHILE FLASHING'
  if (request.type === 'flash-save') return request.save.fileName
  if (request.type === 'backup') return selectedBackupOption.value ? `${selectedBackupOption.value.firmwareName} ${selectedBackupOption.value.version}` : 'Select firmware and version below'
  if (request.type === 'switch-firmware') return `${request.firmwareName} ${request.version}`
  return `${request.firmwareName} ${request.version}`
})
const lcdMessage = computed(() => {
  if (busyAction.value === 'backup') return backupProgress.value === null ? 'BACKUP IN PROGRESS' : `BACKUP ${backupProgress.value}%`
  if (busyAction.value === 'firmware') return flashProgress.value === null ? 'FIRMWARE FLASHING' : `FLASHING ${flashProgress.value}%`
  if (busyAction.value === 'flash') return flashProgress.value === null ? 'BACKUP RESTORING' : `RESTORING ${flashProgress.value}%`
  if (busyAction.value === 'bootloader') return 'FLASHING BOOTLOADER'
  if (lcdStatusError.value) return `ERROR: ${lcdStatusError.value}`
  if (lcdCompletion.value) return lcdCompletion.value
  return props.pendingAction
    ? pendingActionLabel.value
    : detected.value?.downloadMode ? 'DOWNLOAD MODE - READY TO RESTART' : detected.value?.deviceFound ? 'DEVICE READY' : 'WAITING FOR DEVICE'
})
const lcdMessageClass = computed(() => lcdStatusError.value ? 'lcd-message--error' : lcdCompletion.value ? 'lcd-message--complete' : '')
const lcdState = computed<MFlasherLcdState>(() => ({
  title: displayTitle.value,
  message: lcdMessage.value,
  connection: 'USB',
  hint: lcdHint.value,
  progress: busyAction.value === 'backup' ? backupProgress.value : (busyAction.value === 'firmware' || busyAction.value === 'flash' ? flashProgress.value : null),
  messageClass: lcdMessageClass.value,
}))
const lcdHint = computed(() => {
  if (busyAction.value === 'backup') return 'READING FLASH DATA...'
  if (busyAction.value === 'firmware') return 'FLASHING FIRMWARE...'
  if (busyAction.value === 'flash') return 'RESTORING BACKUP...'
  if (busyAction.value === 'bootloader') return 'KEEP DEVICE CONNECTED...'
  if (props.pendingAction && detected.value?.deviceFound) return pendingActionDetail.value
  return detected.value?.downloadMode ? 'RESTART DEVICE. PRESS THE REBOOT BUTTON BELOW' : 'HOLD SEQ REC (SMK-37) OR REC (FM-1) WHILE SWITCHING ON'
})
watch(lcdState, (state) => emit('lcdState', state), { immediate: true })
const primaryActionLabel = computed(() => {
  if (busyAction.value === 'find') return 'CONNECTING…'
  if (busyAction.value === 'download-mode') return 'SWITCHING…'
  if (busyAction.value === 'restart') return 'RESTARTING…'
  if (detected.value?.downloadMode) return 'RESTART'
  if (detected.value?.deviceFound) return 'DOWNLOAD MODE'
  return 'CONNECT'
})

function appendTerminal(command: string, output: string) {
  const stamp = new Date().toLocaleTimeString()
  terminalLines.value.push(`[${stamp}] $ m-flasher ${command}`)
  if (output.trim()) terminalLines.value.push(...output.replace(/\r/g, ' ').trimEnd().split('\n'))
  if (terminalLines.value.length > 300) terminalLines.value.splice(0, terminalLines.value.length - 300)
}

function clearLcdFeedback() {
  lcdCompletion.value = ''
  lcdStatusError.value = ''
}

function showLcdCompletion(message: string) {
  lcdStatusError.value = ''
  lcdCompletion.value = message
}

function clearTerminal() {
  terminalLines.value = []
}

function nativePath(root: string, ...parts: string[]) {
  const separator = root.includes('\\') ? '\\' : '/'
  const cleanedRoot = root.replace(/[\\/]+$/, '')
  const cleanedParts = parts.flatMap((part) => part.split(/[\\/]+/).filter(Boolean))
  return [cleanedRoot, ...cleanedParts].join(separator)
}

function quoteCommandArgument(value: string) {
  return `"${value.replace(/"/g, '\\"')}"`
}

async function showPendingCommand(request: MFlasherRequest) {
  if (shownRequestId === request.requestId) return
  shownRequestId = request.requestId
  if (request.type === 'switch-firmware') switchStage.value = 'choose'
  try {
    const root = request.type === 'flash-bootloader' ? '' : await invoke<string>('get_firmware_folder_path')
    let command: string
    if (request.type === 'flash-bootloader') {
      command = 'm-flasher bootloader --yes'
    } else if (request.type === 'backup') {
      const target = selectedBackupOption.value
      if (!target) throw new Error('Select an installed firmware version before performing a backup.')
      const savesFolder = nativePath(root, target.firmwareId, target.version, 'saves')
      command = `m-flasher backup ${quoteCommandArgument(savesFolder)} --yes`
    } else if (request.type === 'flash-firmware' || request.type === 'switch-firmware') {
      const firmwareFile = nativePath(root, request.firmwareId, request.version, 'firmware.fwsc')
      const flashCommand = `m-flasher firmware ${quoteCommandArgument(firmwareFile)} --yes --restart`
      if (request.type === 'switch-firmware' && request.automaticallyBackup) {
        const target = selectedBackupOption.value
        if (!target) throw new Error('Select the installed firmware version to back up before flashing.')
        const savesFolder = nativePath(root, target.firmwareId, target.version, 'saves')
        command = `m-flasher backup ${quoteCommandArgument(savesFolder)} --yes\n${flashCommand}`
      } else if (request.type === 'switch-firmware') {
        const target = selectedBackupOption.value
        const backupCommand = target
          ? `m-flasher backup ${quoteCommandArgument(nativePath(root, target.firmwareId, target.version, 'saves'))} --yes`
          : 'Select a firmware version to see the backup command.'
        command = `[BACKUP THEN FLASH]\n${backupCommand}\n${flashCommand}\n[SKIP BACKUP]\n${flashCommand}`
      } else {
        command = flashCommand
      }
    } else {
      const file = nativePath(root, request.save.firmwareId, request.save.version, 'saves', request.save.relativePath)
      const operation = request.save.fileName.toLocaleLowerCase().endsWith('.fwsc') ? 'firmware' : 'restore'
      command = `m-flasher ${operation} ${quoteCommandArgument(file)} --yes --restart`
    }
    terminalLines.value.push(`[${new Date().toLocaleTimeString()}] PENDING CONFIRMATION`)
    terminalLines.value.push(`> ${command}`)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    terminalLines.value.push(`[${new Date().toLocaleTimeString()}] Could not prepare command preview: ${message}`)
  }
  if (terminalLines.value.length > 300) terminalLines.value.splice(0, terminalLines.value.length - 300)
}

function markDownloadModeFromOutput(output: string): boolean {
  if (!/in download mode:|storagedevice|physicaldrive|uboot/i.test(output)) return false
  detected.value = {
    output,
    deviceFound: true,
    downloadMode: true,
    deviceName: detected.value?.deviceName ?? null,
  }
  return true
}

function modelFromProduct(product: unknown): string {
  if (typeof product !== 'string') return ''
  if (/smk\s*-?\s*37/i.test(product)) return 'SMK-37'
  if (/fm\s*-?\s*1/i.test(product)) return 'FM-1'
  return ''
}

function matchTarget(firmware: ScannedFirmware | null) {
  if (!firmware) return false
  selectedBackupTarget.value = `${firmware.id}::${firmware.version}`
  localStorage.setItem('fm-mate.lastFlashedFirmwareTarget', selectedBackupTarget.value)
  return true
}

function restoreBackupTarget() {
  if (backupOptions.value.some((option) => option.key === selectedBackupTarget.value)) return
  const remembered = localStorage.getItem('fm-mate.lastFlashedFirmwareTarget') ?? ''
  if (backupOptions.value.some((option) => option.key === remembered)) {
    selectedBackupTarget.value = remembered
    return
  }
  const preferred = [...devices.value].sort((a, b) => {
    const aCurrent = a.version === a.currentVersion
    const bCurrent = b.version === b.currentVersion
    if (aCurrent !== bCurrent) return Number(bCurrent) - Number(aCurrent)
    return Date.parse(b.downloadedAt ?? '') - Date.parse(a.downloadedAt ?? '')
  })[0]
  selectedBackupTarget.value = preferred ? `${preferred.id}::${preferred.version}` : ''
  if (selectedBackupTarget.value) localStorage.setItem('fm-mate.lastFlashedFirmwareTarget', selectedBackupTarget.value)
}

function rememberBackupTarget() {
  if (selectedBackupTarget.value) localStorage.setItem('fm-mate.lastFlashedFirmwareTarget', selectedBackupTarget.value)
}

async function findDevice() {
  if (busyAction.value) return
  busyAction.value = 'find'
  lastError.value = ''
  try {
    if (!isTauri()) throw new Error('M-Flasher device detection is available in the desktop app.')
    devices.value = await invoke<ScannedFirmware[]>('scan_installed_firmwares')
    restoreBackupTarget()
    const result = await invoke<FoundDevice>('m_flasher_find')
    const previous = detected.value
    if (previous?.deviceName !== result.deviceName || previous?.downloadMode !== result.downloadMode) {
      infoFirmware.value = ''
      infoModel.value = ''
    }
    detected.value = result
    appendTerminal('find', result.output || 'No device found.')
    if (!matchTarget(matchingFirmware.value)) restoreBackupTarget()
    if (!result.deviceFound) lastError.value = 'No compatible device was found.'
    if (result.deviceFound && !result.downloadMode) lastError.value = ''
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    detected.value = { output: message, deviceFound: false, downloadMode: false, deviceName: null }
    appendTerminal('find', message)
    lastError.value = message
    lcdStatusError.value = message
  } finally {
    busyAction.value = ''
  }
}

async function runCommand(action: 'restart' | 'download-mode' | 'info') {
  clearLcdFeedback()
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
    if (action === 'download-mode') {
      markDownloadModeFromOutput(output)
    } else if (action === 'restart') {
      await new Promise((resolve) => window.setTimeout(resolve, 1200))
      busyAction.value = ''
      await findDevice()
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    lastError.value = message
    lcdStatusError.value = message
    appendTerminal(action, message)
  } finally {
    busyAction.value = ''
  }
}

async function updateBootloader() {
  clearLcdFeedback()
  busyAction.value = 'info'
  lastError.value = ''
  try {
    const output = await invoke<string>('m_flasher_command', { action: 'info' })
    appendTerminal('info', output)
    
    const model = output.match(/^\s*model\s+(.+)$/im)?.[1]?.trim()
    const firmware = output.match(/^\s*firmware\s+(.+)$/im)?.[1]?.trim()
    if (model) infoModel.value = model
    if (firmware) infoFirmware.value = firmware
    
    const bootloaderMatch = output.match(/^\s*bootloader\s+(.+)$/im)?.[1]?.trim()
    bootloaderInfo.value = bootloaderMatch || ''
    
    if (bootloaderMatch) {
      if (bootloaderMatch.includes('m-bootloader')) {
        confirmationDialog.value = {
          title: 'BOOTLOADER CHECK',
          message: `${bootloaderMatch} already installed, continue anyways?`,
          confirmLabel: 'CONTINUE',
          cancelLabel: 'CANCEL',
          onConfirm: () => {
            confirmationDialog.value = null
            emit('openFlasher', { type: 'flash-bootloader' })
          },
        }
      } else {
        confirmationDialog.value = {
          title: 'BOOTLOADER WARNING',
          message: 'It is seriously recommended that you install m-bootloader to prevent bricking, do you want to install it now?',
          confirmLabel: 'INSTALL',
          cancelLabel: 'CANCEL',
          onConfirm: () => {
            confirmationDialog.value = null
            emit('openFlasher', { type: 'flash-bootloader' })
          },
        }
      }
    } else {
      lastError.value = 'Could not determine bootloader status.'
      lcdStatusError.value = lastError.value
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    lastError.value = message
    lcdStatusError.value = message
    appendTerminal('info', message)
  } finally {
    busyAction.value = ''
  }
}

async function primaryAction() {
  clearLcdFeedback()
  if (!detected.value?.deviceFound) {
    await findDevice()
  } else {
    await runCommand(detected.value.downloadMode ? 'restart' : 'download-mode')
  }
}

async function backupDevice(): Promise<boolean> {
  const [firmwareId, version] = selectedBackupTarget.value.split('::')
  if (!firmwareId || !version) {
    const message = 'Select an installed firmware version to associate with this backup.'
    lastError.value = message
    lcdStatusError.value = message
    return false
  }
  busyAction.value = 'backup'
  clearLcdFeedback()
  lastError.value = ''
  backupProgress.value = null
  const operationId = `backup-${Date.now()}-${Math.random().toString(36).slice(2)}`
  let unlisten: (() => void) | undefined
  try {
    if (!detected.value?.downloadMode) {
      busyAction.value = 'download-mode'
      const modeOutput = await invoke<string>('m_flasher_command', { action: 'download-mode' })
      appendTerminal('download-mode', modeOutput)
      if (!markDownloadModeFromOutput(modeOutput)) {
        throw new Error('M-Flasher did not report that the device entered download mode. Backup was not started.')
      }
      busyAction.value = 'backup'
    }
    backupProgress.value = 0
    unlisten = await listen<{ operationId: string; output: string; progress: number | null }>('mflasher-backup-progress', (event) => {
      if (event.payload.operationId === operationId && event.payload.progress !== null) {
        backupProgress.value = event.payload.progress
      }
    })
    const output = await invoke<string>('m_flasher_backup', { firmwareId, version, operationId })
    backupProgress.value = 100
    appendTerminal('backup', output)
    showLcdCompletion('BACKUP COMPLETED!')
    return true
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    backupProgress.value = null
    lastError.value = message
    lcdStatusError.value = message
    appendTerminal('backup', message)
    return false
  } finally {
    unlisten?.()
    busyAction.value = ''
  }
}

async function flashFirmwareRequest(request: Extract<MFlasherRequest, { type: 'flash-firmware' | 'switch-firmware' }>): Promise<boolean> {
  busyAction.value = 'firmware'
  clearLcdFeedback()
  lastError.value = ''
  const operationId = `flash-${Date.now()}-${Math.random().toString(36).slice(2)}`
  let unlisten: (() => void) | undefined
  try {
    flashProgress.value = 0
    unlisten = await listen<{ operationId: string; output: string; progress: number | null }>('mflasher-flash-progress', (event) => {
      if (event.payload.operationId === operationId && event.payload.progress !== null) {
        flashProgress.value = event.payload.progress
      }
    })
    const output = await invoke<string>('m_flasher_flash_firmware', {
      firmwareId: request.firmwareId,
      version: request.version,
      operationId,
    })
    flashProgress.value = 100
    appendTerminal(`firmware ${request.firmwareName} ${request.version}`, output)
    localStorage.setItem('fm-mate.lastFlashedFirmwareTarget', `${request.firmwareId}::${request.version}`)
    showLcdCompletion('FIRMWARE FLASHED!')
    return true
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    flashProgress.value = null
    lastError.value = message
    lcdStatusError.value = message
    appendTerminal('firmware', message)
    return false
  } finally {
    unlisten?.()
    busyAction.value = ''
  }
}

async function backupBeforeSwitch() {
  const request = props.pendingAction
  if (request?.type !== 'switch-firmware') return
  if (await prepareSwitchBackup()) switchStage.value = 'flash'
}

async function prepareSwitchBackup(): Promise<boolean> {
  if (!detected.value?.deviceFound) await findDevice()
  if (!detected.value?.deviceFound) {
    const message = 'Connect a device before backing it up for the firmware switch.'
    lastError.value = message
    lcdStatusError.value = message
    return false
  }
  return backupDevice()
}

function skipSwitchBackup() {
  switchStage.value = 'flash'
  terminalLines.value.push(`[${new Date().toLocaleTimeString()}] Backup skipped by user.`)
}

async function flashAfterSwitchBackup() {
  const request = props.pendingAction
  if (request?.type !== 'switch-firmware' || switchStage.value !== 'flash') return
  if (await flashFirmwareRequest(request)) {
    await findDevice()
    emit('actionComplete', request.requestId)
  }
}

async function confirmBackupAndFlash() {
  const request = props.pendingAction
  if (request?.type !== 'switch-firmware' || !request.automaticallyBackup) return
  if (!await prepareSwitchBackup()) return
  if (await flashFirmwareRequest(request)) {
    await findDevice()
    emit('actionComplete', request.requestId)
  }
}

async function handlePendingAction(request: MFlasherRequest) {
  if (request.requestId === handledRequestId) return
  handledRequestId = request.requestId
  clearLcdFeedback()
  lastError.value = ''
  let unlisten: (() => void) | undefined
  try {
    if (request.type === 'flash-bootloader') {
      busyAction.value = 'bootloader'
      const output = await invoke<string>('m_flasher_flash_bootloader')
      appendTerminal('bootloader --yes', output)
      showLcdCompletion('BOOTLOADER FLASHED!')
    } else if (request.type === 'flash-firmware') {
      await flashFirmwareRequest(request)
    } else if (request.type === 'flash-save') {
      busyAction.value = 'flash'
      const operationId = `flash-${Date.now()}-${Math.random().toString(36).slice(2)}`
      flashProgress.value = 0
      unlisten = await listen<{ operationId: string; output: string; progress: number | null }>('mflasher-flash-progress', (event) => {
        if (event.payload.operationId === operationId && event.payload.progress !== null) {
          flashProgress.value = event.payload.progress
        }
      })
      const output = await invoke<string>('m_flasher_flash_save', {
        firmwareId: request.save.firmwareId,
        version: request.save.version,
        relativePath: request.save.relativePath,
        operationId,
      })
      flashProgress.value = 100
      appendTerminal(`flash ${request.save.fileName}`, output)
      showLcdCompletion(request.save.fileName.toLocaleLowerCase().endsWith('.fwsc') ? 'FIRMWARE FLASHED!' : 'BACKUP FLASHED!')
    } else if (request.type === 'backup') {
      await findDevice()
      if (!detected.value?.deviceFound) throw new Error('Connect a device before saving a backup.')
      if (!selectedBackupTarget.value) throw new Error('Select an installed firmware version before performing a backup.')
      await backupDevice()
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    lastError.value = message
    lcdStatusError.value = message
    appendTerminal(request.type, message)
    if (busyAction.value === 'flash') flashProgress.value = null
  } finally {
    unlisten?.()
    busyAction.value = ''
    if (!lastError.value) await findDevice()
    emit('actionComplete', request.requestId)
  }
}

function activateView() {
  clearLcdFeedback()
  void (async () => {
    await findDevice()
    if (props.pendingAction) await showPendingCommand(props.pendingAction)
  })()
}

onActivated(activateView)
watch(() => props.pendingAction, (request) => {
  if (request?.type === 'flash-bootloader') void showPendingCommand(request)
})

function confirmPendingAction() {
  if (props.pendingAction) void handlePendingAction(props.pendingAction)
}

function cancelPendingAction() {
  if (props.pendingAction) {
    terminalLines.value.push(`[${new Date().toLocaleTimeString()}] Action cancelled.`)
    emit('actionComplete', props.pendingAction.requestId)
  }
}
</script>

<template>
  <section class="view-panel lcd-view">
    <div class="view-heading">
      <div><p class="eyebrow">DEVICE STATUS</p><h1>M-Flasher</h1></div>
      <span class="connection-pill" :class="{ 'connection-pill--online': detected?.deviceFound }"><i /> {{ connectionStatus }}</span>
    </div>
    <div class="mflasher-content">
    <div class="lcd-card">
      <LcdDisplay v-bind="lcdState" />
      <div v-if="!detected?.deviceFound" class="lcd-actions">
        <button class="action-button" type="button" :disabled="!!busyAction" @click="primaryAction">{{ primaryActionLabel }}</button>
        <button v-if="pendingAction" class="outline-button pending-cancel" type="button" :disabled="!!busyAction" @click="cancelPendingAction">CANCEL</button>
      </div>
      <div v-else-if="pendingAction?.type === 'switch-firmware'" class="pending-actions pending-actions--switch">
        <template v-if="pendingAction.automaticallyBackup">
          <button class="action-button action-button--primary" type="button" :disabled="!!busyAction || !pendingActionReady" @click="confirmBackupAndFlash">BACKUP &amp; FLASH</button>
          <button class="outline-button pending-cancel" type="button" :disabled="!!busyAction" @click="cancelPendingAction">CANCEL</button>
        </template>
        <template v-else-if="switchStage === 'choose'">
          <button class="action-button action-button--primary" type="button" :disabled="!!busyAction || !selectedBackupTarget" @click="backupBeforeSwitch">BACKUP THEN FLASH</button>
          <button class="action-button" type="button" :disabled="!!busyAction" @click="skipSwitchBackup">SKIP BACKUP</button>
          <button class="outline-button pending-cancel" type="button" :disabled="!!busyAction" @click="cancelPendingAction">CANCEL</button>
        </template>
        <template v-else>
          <button class="action-button action-button--primary" type="button" :disabled="!!busyAction" @click="flashAfterSwitchBackup">FLASH FIRMWARE</button>
          <button class="outline-button pending-cancel" type="button" :disabled="!!busyAction" @click="cancelPendingAction">CANCEL</button>
        </template>
      </div>
      <div v-else-if="pendingAction" class="pending-actions">
        <button class="action-button action-button--primary" type="button" :disabled="!!busyAction || !pendingActionReady" @click="confirmPendingAction">{{ pendingActionLabel }}</button>
        <button v-if="pendingAction.type === 'flash-save' && !pendingAction.save.fileName.toLocaleLowerCase().endsWith('.fwsc')" class="action-button" type="button" :disabled="!!busyAction || !detected?.deviceFound || !selectedBackupTarget" @click="backupDevice">{{ busyAction === 'backup' ? 'SAVING…' : 'SAVE BACKUP' }}</button>
        <button class="outline-button pending-cancel" type="button" :disabled="!!busyAction" @click="cancelPendingAction">CANCEL</button>
      </div>
      <div v-else class="lcd-actions">
        <button class="action-button" type="button" :disabled="!!busyAction" @click="primaryAction">{{ primaryActionLabel }}</button>
        <button class="action-button" type="button" :disabled="!!busyAction || !detected?.deviceFound || !selectedBackupTarget" @click="backupDevice">{{ busyAction === 'backup' ? 'SAVING…' : 'BACKUP' }}</button>
        <button class="action-button" type="button" :disabled="!!busyAction || !detected?.deviceFound" @click="updateBootloader">{{ busyAction === 'info' ? 'READING…' : 'Flash m-bootloader' }}</button>
      </div>
      <label class="backup-firmware-picker">
        FIRMWARE TO ASSOCIATE WITH THIS SAVE
        <select v-model="selectedBackupTarget" :disabled="!!busyAction || !backupOptions.length" @change="rememberBackupTarget">
          <option value="">Select installed firmware version</option>
          <option v-for="option in backupOptions" :key="option.key" :value="option.key">{{ option.firmwareName }} — {{ option.version }}</option>
        </select>
      </label>
      <p v-if="lastError" class="lcd-error" role="status">{{ lastError }}</p>
      <p class="lcd-caption">LIVE DEVICE DISPLAY <span>—</span> {{ detected?.deviceFound ? 'Connected through M-Flasher.' : 'Checking for a compatible device.' }}</p>
    </div>
    <section class="mflasher-terminal" aria-label="M-Flasher terminal output">
      <div class="terminal-heading">
        <h2>M-FLASHER TERMINAL</h2>
        <button class="outline-button terminal-clear" type="button" :disabled="!terminalLines.length" @click="clearTerminal">CLEAR TERMINAL</button>
      </div>
      <pre ref="terminalOutput" class="terminal-output" aria-live="polite">{{ terminalLines.join('\n') || 'Waiting for M-Flasher output…' }}</pre>
    </section>
    </div>
  </section>
  <ConfirmationDialog 
    v-if="confirmationDialog"
    :title="confirmationDialog.title"
    :message="confirmationDialog.message"
    :confirmLabel="confirmationDialog.confirmLabel"
    :cancelLabel="confirmationDialog.cancelLabel"
    @confirm="confirmationDialog.onConfirm"
    @secondary="confirmationDialog = null"
    @cancel="confirmationDialog = null"
  />
</template>

<style>
.lcd-view .connection-pill { display: inline-flex; align-items: center; gap: 8px; }
.lcd-view .connection-pill i { display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: #988b8b; }
.lcd-view .connection-pill--online i { background: #68e1be; box-shadow: 0 0 8px #68e1be88; }
.lcd-view { display: flex; flex-direction: column; height: 100vh; padding-top: clamp(25px, 4vw, 48px) !important; }
.mflasher-content { display: flex; flex-wrap: nowrap; align-items: stretch; gap: 18px; max-height: calc(100vh - 180px); overflow: hidden; flex: 1; }
.mflasher-content .lcd-card { flex: 0 0 auto; min-width: 0; }
.lcd-card { width: 100%; max-width: 980px; padding: clamp(16px, 3vw, 28px); background: #282524; border: 1px solid #48413f; display: flex; flex-direction: column; overflow: auto; }
.lcd-card .lcd-message { font-size: clamp(13px, 1.65vw, 20px); line-height: 1.1; overflow-wrap: anywhere; }
.lcd-card .lcd-message--error { display: -webkit-box; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; font-size: clamp(11px, 1.35vw, 16px); }
.lcd-actions { display: flex; gap: 10px; margin-top: 15px; }
.lcd-actions .action-button { flex: 1; justify-content: center; min-height: 48px; }
.pending-actions { display: flex; gap: 10px; margin-top: 15px; }
.pending-actions .action-button { flex: 1; justify-content: center; min-height: 58px; }
.pending-cancel { min-width: 145px; min-height: 58px; }
.backup-firmware-picker { display: grid; gap: 6px; margin-top: 14px; color: #b49bdc; font-size: 10px; letter-spacing: .13em; }
.backup-firmware-picker select { padding: 9px 10px; color: #e3dcd2; background: #282525; border: 1px solid #514754; font: 12px 'Share Tech Mono', monospace; }
.lcd-error { margin: 12px 0 0; color: #efaa9e; font: 12px 'Share Tech Mono', monospace; }
.lcd-caption { margin: 14px 0 0; color: #b49bdc; font: 11px 'Share Tech Mono', monospace; letter-spacing: .12em; }
.lcd-caption span { color: #73dfc1; }
.mflasher-terminal { flex: 1 1 340px; min-width: 340px; max-width: 980px; border: 1px solid #514754; background: #211f20; display: flex; flex-direction: column; overflow: hidden; }
.terminal-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 9px 12px; border-bottom: 1px solid #514754; }
.terminal-heading h2 { margin: 0; color: #b49bdc; font-size: 12px; letter-spacing: .16em; }
.terminal-clear { min-height: 30px; padding: 4px 10px; font-size: 10px; letter-spacing: .1em; }
.terminal-output { flex: 1; overflow: auto; margin: 0; padding: 12px; color: #b9d6c9; background: #171718; font: 12px/1.55 'Share Tech Mono', monospace; white-space: pre-wrap; overflow-wrap: anywhere; }
@media (max-width: 560px) { .lcd-actions, .pending-actions { flex-direction: column; } .pending-cancel { min-width: 0; } .mflasher-content { flex-direction: column; max-height: none; } .mflasher-terminal { max-width: 100%; min-width: auto; } }
</style>
