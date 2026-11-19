<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";

const updateInfo = ref<{
  isUpdateAvailable: boolean;
  currentVersion: string;
  latestVersion?: string;
} | null>(null);
const checkingUpdates = ref(false);
const updateMessage = ref("");
const updateError = ref(false);

onMounted(() => {
  if (isTauri()) void checkForUpdates();
});

async function checkForUpdates() {
  if (!isTauri()) return;
  checkingUpdates.value = true;
  updateMessage.value = "";
  updateError.value = false;
  try {
    const info = await invoke<{
      isUpdateAvailable: boolean;
      currentVersion: string;
      latestVersion?: string;
    }>("check_app_update");
    updateInfo.value = info;
    if (info.isUpdateAvailable) {
      updateMessage.value = `A new version ${info.latestVersion} is available! Visit GitHub to download the latest release.`;
      updateError.value = false;
    } else {
      updateMessage.value = `You are running the latest version (${info.currentVersion}).`;
      updateError.value = false;
    }
  } catch (error) {
    updateError.value = true;
    if (typeof error === "string") updateMessage.value = error;
    else if (error instanceof Error) updateMessage.value = error.message;
    else if (
      error &&
      typeof error === "object" &&
      "message" in error &&
      typeof error.message === "string"
    )
      updateMessage.value = error.message;
    else updateMessage.value = "Could not check for updates.";
  } finally {
    checkingUpdates.value = false;
  }
}
</script>

<template>
  <section class="settings-group app-update">
    <h2>APP UPDATES</h2>
    <p class="repo-description">Check for new versions of FM-MATE.</p>
    <button
      class="outline-button"
      :disabled="!isTauri() || checkingUpdates"
      @click="checkForUpdates"
    >
      {{ checkingUpdates ? "CHECKING..." : "CHECK FOR UPDATES" }}
    </button>
    <p v-if="!isTauri()" class="settings-feedback">
      Update checks are available in the desktop app.
    </p>
    <p
      v-if="updateMessage"
      class="settings-feedback"
      :class="{ 'is-error': updateError }"
      role="status"
    >
      {{ updateMessage }}
    </p>
    <p
      v-if="updateInfo && updateInfo.isUpdateAvailable"
      class="settings-feedback"
    >
      <a
        href="https://github.com/EJTH/FM-MATE/releases/latest"
        target="_blank"
        rel="noopener noreferrer"
      >
        Download {{ updateInfo.latestVersion }} →
      </a>
    </p>
  </section>
</template>

<style scoped>
.app-update {
  border-top: 1px solid var(--color-border);
  padding-top: 1.5rem;
  margin-top: 1.5rem;
}

.settings-feedback a {
  color: var(--color-link);
  text-decoration: none;
}

.settings-feedback a:hover {
  text-decoration: underline;
}
</style>
