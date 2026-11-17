<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import ToolDownloadCard from './ToolDownloadCard.vue'

withDefaults(defineProps<{ title?: string; description?: string }>(), {
  title: 'EXTRA DOWNLOADS',
  description: "Optional and required utilities for working with firmware. Downloads are kept in the app's tools folder.",
})

const tools = [
  { id: 'm-flasher', name: 'M-Flasher', description: 'Required to flash new firmwares and perform backups.', repository: 'jvitkauskas/m-flasher' },
  { id: 'fm1-emu', name: 'FM1-Emu', description: 'Optional; lets you test firmwares locally before installing them on your device.', repository: 'joakimeriksson/fm1-emu' },
]
const toolStatuses = ref<Record<string, { installed: boolean; version: string | null }>>({})
const toolBusyId = ref('')
const toolMessage = ref('')
const toolError = ref(false)

onMounted(() => { if (isTauri()) void loadTools() })

async function loadTools() {
  try {
    const statuses = await invoke<Array<{ id: string; installed: boolean; version: string | null }>>('get_tools_status')
    toolStatuses.value = Object.fromEntries(statuses.map((tool) => [tool.id, { installed: tool.installed, version: tool.version }]))
  } catch (error) {
    toolError.value = true
    toolMessage.value = error instanceof Error ? error.message : 'Could not check downloaded tools.'
  }
}

async function runToolAction(id: string) {
  if (!isTauri()) return
  toolBusyId.value = id
  toolMessage.value = ''
  toolError.value = false
  try {
    const result = await invoke<{ status: 'downloaded' | 'updated' | 'up-to-date'; version: string }>('check_or_update_tool', { id })
    const name = tools.find((tool) => tool.id === id)?.name ?? id
    if (result.status === 'up-to-date') toolMessage.value = `${name} is up to date (${result.version}).`
    else if (result.status === 'updated') toolMessage.value = `Updated ${name} to ${result.version}.`
    else toolMessage.value = `Downloaded ${name} ${result.version}.`
    await loadTools()
  } catch (error) {
    toolError.value = true
    toolMessage.value = error instanceof Error ? error.message : 'Could not download the tool.'
  } finally {
    toolBusyId.value = ''
  }
}
</script>

<template>
  <section class="settings-group extra-downloads">
    <h2>{{ title }}</h2>
    <p class="repo-description">{{ description }}</p>
    <ToolDownloadCard
      v-for="tool in tools"
      :key="tool.id"
      :name="tool.name"
      :description="tool.description"
      :repository="tool.repository"
      :installed="toolStatuses[tool.id]?.installed ?? false"
      :version="toolStatuses[tool.id]?.version"
      :busy="toolBusyId === tool.id"
      :disabled="!isTauri()"
      @action="runToolAction(tool.id)"
    />
    <p v-if="!isTauri()" class="settings-feedback">Tool downloads are available in the desktop app.</p>
    <p v-if="toolMessage" class="settings-feedback" :class="{ 'is-error': toolError }" role="status">{{ toolMessage }}</p>
  </section>
</template>
