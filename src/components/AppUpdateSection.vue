<script setup lang="ts">
import { onMounted, ref } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getAppRelease, type AppUpdateInfo } from "../services/appRelease";

const updateInfo = ref<AppUpdateInfo | null>(null);
const releaseUrl = ref<string | null>(null);
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
    const release = await getAppRelease();
    const info = release.updateInfo;
    if (!info) throw new Error("Update information is unavailable.");
    updateInfo.value = info;
    releaseUrl.value = release.htmlUrl;
    if (info.isUpdateAvailable) {
      updateMessage.value = `A new version ${info.latestVersion} is available.`;
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

async function openRelease() {
  try {
    await openUrl(
      releaseUrl.value || "https://github.com/EJTH/FM-MATE/releases/latest",
    );
  } catch (error) {
    console.error("Could not open the app download page:", error);
    updateError.value = true;
    updateMessage.value = "Could not open the download page.";
  }
}

async function handleUpdateButtonClick() {
  if (updateInfo.value?.isUpdateAvailable) {
    await openRelease();
    return;
  }
  await checkForUpdates();
}
</script>

<template>
  <section class="settings-group app-update">
    <h2>APP UPDATES</h2>
    <p class="repo-description">
      Update information is checked on app startup. Restart the app to check again.
    </p>
    <button
      class="outline-button"
      :disabled="!isTauri() || checkingUpdates"
      @click="handleUpdateButtonClick"
    >
      {{
        checkingUpdates
          ? "CHECKING..."
          : updateInfo?.isUpdateAvailable
            ? `Download ${updateInfo.latestVersion} →`
            : "CHECK FOR UPDATES"
      }}
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
  </section>
</template>

<style scoped>
.app-update {
  border-top: 1px solid var(--color-border);
  padding-top: 1.5rem;
  margin-top: 1.5rem;
}

</style>
