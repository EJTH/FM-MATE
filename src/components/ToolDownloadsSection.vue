<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import ToolDownloadCard from "./ToolDownloadCard.vue";
import Toast from "./Toast.vue";

withDefaults(defineProps<{ title?: string; description?: string }>(), {
  title: "EXTRA DOWNLOADS",
  description:
    "Optional and required utilities for working with firmware. Downloads are kept in the app's tools folder.",
});
const emit = defineEmits<{ mFlasherStatus: [installed: boolean | null] }>();

const tools = [
  {
    id: "m-flasher",
    name: "M-Flasher",
    description: "Required to flash new firmwares and perform backups.",
    repository: "jvitkauskas/m-flasher",
  },
  {
    id: "fm1-emu",
    name: "FM1-Emu",
    description:
      "Optional; lets you test firmwares locally before installing them on your device.",
    repository: "joakimeriksson/fm1-emu",
  },
];
const toolStatuses = ref<
  Record<string, { installed: boolean; version: string | null }>
>({});
const toolBusyId = ref("");
const toast = ref<{
  message: string;
  variant: "success" | "error" | "info";
} | null>(null);

onMounted(() => {
  if (isTauri()) void loadTools();
});

async function loadTools() {
  try {
    const statuses =
      await invoke<
        Array<{ id: string; installed: boolean; version: string | null }>
      >("get_tools_status");
    const mFlasher = statuses.find((tool) => tool.id === "m-flasher");
    if (!mFlasher) throw new Error("M-Flasher status was not returned.");
    toolStatuses.value = Object.fromEntries(
      statuses.map((tool) => [
        tool.id,
        { installed: tool.installed, version: tool.version },
      ]),
    );
    emit("mFlasherStatus", mFlasher.installed);
  } catch (error) {
    emit("mFlasherStatus", null);
    toast.value = {
      message:
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : "Could not check downloaded tools.",
      variant: "error",
    };
  }
}

async function runToolAction(id: string) {
  if (!isTauri()) return;
  toolBusyId.value = id;
  const name = tools.find((tool) => tool.id === id)?.name ?? id;
  toast.value = {
    message: `Checking or downloading ${name}...`,
    variant: "info",
  };
  try {
    const result = await invoke<{
      status: "downloaded" | "updated" | "up-to-date";
      version: string;
    }>("check_or_update_tool", { id });
    if (result.status === "up-to-date")
      toast.value = {
        message: `${name} is up to date (${result.version}).`,
        variant: "info",
      };
    else if (result.status === "updated")
      toast.value = {
        message: `Updated ${name} to ${result.version}.`,
        variant: "success",
      };
    else
      toast.value = {
        message: `Downloaded ${name} ${result.version}.`,
        variant: "success",
      };
    await loadTools();
  } catch (error) {
    toast.value = {
      message:
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : "Could not download the tool.",
      variant: "error",
    };
  } finally {
    toolBusyId.value = "";
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
    <p v-if="!isTauri()" class="settings-feedback">
      Tool downloads are available in the desktop app.
    </p>
    <Toast
      v-if="toast"
      :message="toast.message"
      :variant="toast.variant"
      :reset-key="toast"
      @dismiss="toast = null"
    />
  </section>
</template>
