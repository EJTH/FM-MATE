<script setup lang="ts">
import { ref } from 'vue'
import type { InstalledSave } from '../types/saves'
import ConfirmationDialog from './ConfirmationDialog.vue'

defineProps<{
  saves: InstalledSave[]
  loading: boolean
  loadError: boolean
  loadErrorMessage: string
  emptyMessage: string
  emulating?: boolean
}>()
const emit = defineEmits<{
  emulate: [save: InstalledSave]
  openFolder: [save: InstalledSave]
  flash: [save: InstalledSave]
  delete: [save: InstalledSave]
}>()
const pendingDelete = ref<InstalledSave | null>(null)

function formattedDate(timestamp: number) {
  if (!timestamp) return 'Unknown'
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(timestamp)
}

function formattedSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`
}

function canFlash(save: InstalledSave) {
  const fileName = save.fileName.toLocaleLowerCase()
  return fileName.endsWith('.bin') || fileName.endsWith('.fwsc')
}

function flashTitle(save: InstalledSave) {
  if (save.fileName.toLocaleLowerCase().endsWith('.fwsc')) return 'Install firmware package to device'
  if (canFlash(save)) return 'Restore backup to device'
  return 'Only M-Flasher .bin backups and .fwsc packages can be flashed'
}

function deleteSave(save: InstalledSave) {
  pendingDelete.value = save
}

function confirmDeleteSave() {
  if (pendingDelete.value) emit('delete', pendingDelete.value)
  pendingDelete.value = null
}
</script>

<template>
  <div class="saves-table-wrap">
    <table class="saves-table">
      <thead><tr><th scope="col">FILENAME</th><th scope="col">DATE</th><th scope="col">SIZE</th><th scope="col" class="save-actions-heading">ACTIONS</th></tr></thead>
      <tbody v-if="!loading && !loadError && saves.length">
        <tr v-for="save in saves" :key="`${save.firmwareId}/${save.version}/${save.relativePath}`">
          <td><strong>{{ save.fileName }}</strong><small>{{ save.firmwareName }} · {{ save.version }}</small></td>
          <td>{{ formattedDate(save.modifiedAt) }}</td>
          <td>{{ formattedSize(save.size) }}</td>
          <td class="save-action-cell">
            <div class="save-action-buttons">
              <button class="save-icon-button save-emulate-button" type="button" :disabled="emulating" :aria-label="`Emulate ${save.fileName}`" title="Emulate" @click="emit('emulate', save)">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13L19 12 8 5.5Z" /></svg>
                <span class="sr-only">Emulate</span>
              </button>
              <button class="save-icon-button save-folder-button" type="button" :aria-label="`Open folder for ${save.fileName}`" title="Open save folder" @click="emit('openFolder', save)">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6.5h7l2 2h9v10.5H3zM3 9h18" /></svg>
                <span class="sr-only">Open save folder</span>
              </button>
              <button class="save-icon-button save-flash-button" type="button" :aria-label="`Flash ${save.fileName}`" :title="flashTitle(save)" :disabled="!canFlash(save)" @click="emit('flash', save)">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 15V3m0 0L7.5 7.5M12 3l4.5 4.5M5 14v6h14v-6" /></svg>
                <span class="sr-only">Flash</span>
              </button>
              <button class="save-icon-button save-delete-button" type="button" :aria-label="`Delete ${save.fileName}`" title="Delete" @click="deleteSave(save)">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16m-10 4v7m4-7v7M6 7l1 14h10l1-14M9 7V4h6v3" /></svg>
                <span class="sr-only">Delete</span>
              </button>
            </div>
          </td>
        </tr>
      </tbody>
    </table>
    <div v-if="loading" class="saves-empty"><h2>Scanning installed saves…</h2></div>
    <div v-else-if="loadError" class="saves-empty"><h2>Could not read installed saves</h2><p>{{ loadErrorMessage || 'Check that the firmware folder is available, then return to this tab to retry.' }}</p></div>
    <div v-else-if="!saves.length" class="saves-empty"><span class="empty-mark">▤</span><h2>No saves found</h2><p>{{ emptyMessage }}</p></div>
  </div>
  <ConfirmationDialog
    v-if="pendingDelete"
    title="DELETE SAVE?"
    :message="`Delete ${pendingDelete.fileName} from ${pendingDelete.firmwareName} ${pendingDelete.version}?`"
    confirm-label="DELETE SAVE"
    confirm-variant="danger"
    @confirm="confirmDeleteSave"
    @secondary="pendingDelete = null"
    @cancel="pendingDelete = null"
  />
</template>

<style>
.saves-table-wrap { min-height: 260px; overflow-x: auto; background: rgb(40 37 36 / 70%); border: 1px solid #48413f; }
.saves-table { width: 100%; border-collapse: collapse; color: #ddd5ca; font: 12px 'Share Tech Mono', monospace; text-align: left; }
.saves-table th { padding: 12px 14px; color: #b49bdc; background: #282525; border-bottom: 1px solid #514754; font-size: 10px; letter-spacing: .14em; white-space: nowrap; }
.saves-table td { padding: 11px 14px; border-bottom: 1px solid #403b3a; white-space: nowrap; }
.saves-table tbody tr:last-child td { border-bottom: 0; }.saves-table tbody tr:hover { background: rgb(255 255 255 / 3%); }
.saves-table td:first-child { min-width: 230px; white-space: normal; }.saves-table td:first-child strong, .saves-table td:first-child small { display: block; overflow-wrap: anywhere; }
.saves-table td:first-child strong { color: #e6dfd5; font-weight: 600; }.saves-table td:first-child small { margin-top: 4px; color: #9c9290; font-size: 10px; }
.save-actions-heading, .save-action-cell { width: 1%; text-align: right; }
.save-action-buttons { display: inline-flex; align-items: center; gap: 6px; }
.save-icon-button { display: grid; width: 32px; height: 30px; place-items: center; border: 1px solid; background: transparent; cursor: pointer; }
.save-icon-button svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
.save-emulate-button { color: #b6e5d8; background: #203633; border-color: #3b7e72; }.save-emulate-button svg { fill: currentColor; stroke: none; }.save-emulate-button:hover { color: #e4fff7; border-color: #65c8b3; }
.save-folder-button { color: #e4c988; background: #393324; border-color: #8b7240; }.save-folder-button:hover { color: #fff1cb; border-color: #c5a35d; }
.save-flash-button { color: #b9c9ec; background: #252f42; border-color: #536c9d; }.save-flash-button:hover:not(:disabled) { color: #e1eaff; border-color: #849dd0; }.save-flash-button:disabled { opacity: .35; cursor: not-allowed; }
.save-delete-button { color: #efaaa3; background: #382625; border-color: #80504b; }.save-delete-button:hover { color: #ffe2df; background: #4a2a28; border-color: #b56b63; }
.save-icon-button:focus-visible { outline: 2px solid #65e1c1; outline-offset: 2px; }
.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
.saves-empty { min-height: 260px; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 25px; text-align: center; }
.saves-empty h2 { margin: 0 0 5px; color: #e3dcd2; font-size: 20px; font-weight: 600; }.saves-empty p { margin: 0; color: #a49b97; font: 13px 'Share Tech Mono', monospace; }
@media (max-width: 450px) { .saves-table th, .saves-table td { padding: 9px 8px; }.saves-table { font-size: 11px; } }
</style>
