<script setup lang="ts">
import { ref } from 'vue'
import type { InstalledSave } from '../types/saves'

defineProps<{
  saves: InstalledSave[]
  loading: boolean
  loadError: boolean
  emptyMessage: string
}>()
const emit = defineEmits<{ delete: [save: InstalledSave] }>()
const flashMessage = ref('')

function formattedDate(timestamp: number) {
  if (!timestamp) return 'Unknown'
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(timestamp)
}

function formattedSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`
}

function flashSave(save: InstalledSave) {
  flashMessage.value = `Connect an FM-1 or SMK-37 to flash ${save.fileName}.`
}

function deleteSave(save: InstalledSave) {
  if (!window.confirm(`Delete save “${save.fileName}” from ${save.firmwareName} ${save.version}?`)) return
  emit('delete', save)
}
</script>

<template>
  <div class="saves-table-wrap">
    <p v-if="flashMessage" class="saves-table-message" role="status">{{ flashMessage }}</p>
    <table class="saves-table">
      <thead><tr><th scope="col">FILENAME</th><th scope="col">DATE</th><th scope="col">SIZE</th><th scope="col" class="save-actions-heading">FLASH</th><th scope="col" class="save-actions-heading">DELETE</th></tr></thead>
      <tbody v-if="!loading && !loadError && saves.length">
        <tr v-for="save in saves" :key="`${save.firmwareId}/${save.version}/${save.relativePath}`">
          <td><strong>{{ save.fileName }}</strong><small>{{ save.firmwareName }} · {{ save.version }}</small></td>
          <td>{{ formattedDate(save.modifiedAt) }}</td>
          <td>{{ formattedSize(save.size) }}</td>
          <td class="save-action-cell"><button class="save-flash-button" type="button" @click="flashSave(save)">FLASH</button></td>
          <td class="save-action-cell"><button class="save-delete-button" type="button" @click="deleteSave(save)">DELETE</button></td>
        </tr>
      </tbody>
    </table>
    <div v-if="loading" class="saves-empty"><h2>Scanning installed saves…</h2></div>
    <div v-else-if="loadError" class="saves-empty"><h2>Could not read installed saves</h2><p>Check that the firmware folder is available, then return to this tab to retry.</p></div>
    <div v-else-if="!saves.length" class="saves-empty"><span class="empty-mark">▤</span><h2>No saves found</h2><p>{{ emptyMessage }}</p></div>
  </div>
</template>

<style>
.saves-table-wrap { min-height: 260px; overflow-x: auto; background: rgb(40 37 36 / 70%); border: 1px solid #48413f; }
.saves-table-message { margin: 0; padding: 9px 14px; color: #edb58d; border-bottom: 1px solid #48413f; font: 12px 'Share Tech Mono', monospace; }
.saves-table { width: 100%; border-collapse: collapse; color: #ddd5ca; font: 12px 'Share Tech Mono', monospace; text-align: left; }
.saves-table th { padding: 12px 14px; color: #b49bdc; background: #282525; border-bottom: 1px solid #514754; font-size: 10px; letter-spacing: .14em; white-space: nowrap; }
.saves-table td { padding: 11px 14px; border-bottom: 1px solid #403b3a; white-space: nowrap; }
.saves-table tbody tr:last-child td { border-bottom: 0; }.saves-table tbody tr:hover { background: rgb(255 255 255 / 3%); }
.saves-table td:first-child { min-width: 230px; white-space: normal; }.saves-table td:first-child strong, .saves-table td:first-child small { display: block; overflow-wrap: anywhere; }
.saves-table td:first-child strong { color: #e6dfd5; font-weight: 600; }.saves-table td:first-child small { margin-top: 4px; color: #9c9290; font-size: 10px; }
.save-actions-heading, .save-action-cell { width: 1%; text-align: right; }.save-action-cell button { padding: 5px 9px; border: 1px solid; font: 10px 'Share Tech Mono', monospace; letter-spacing: .06em; cursor: pointer; }
.save-flash-button { color: #b6e5d8; background: #203633; border-color: #3b7e72 !important; }.save-flash-button:hover { color: #e4fff7; border-color: #65c8b3 !important; }
.save-delete-button { color: #efaaa3; background: #382625; border-color: #80504b !important; }.save-delete-button:hover { color: #ffe2df; background: #4a2a28; border-color: #b56b63 !important; }
.saves-empty { min-height: 260px; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 25px; text-align: center; }
.saves-empty h2 { margin: 0 0 5px; color: #e3dcd2; font-size: 20px; font-weight: 600; }.saves-empty p { margin: 0; color: #a49b97; font: 13px 'Share Tech Mono', monospace; }
@media (max-width: 650px) { .saves-table th, .saves-table td { padding: 9px 8px; }.saves-table { font-size: 11px; } }
</style>
