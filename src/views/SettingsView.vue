<script setup lang="ts">
import { computed, onActivated, onMounted, reactive, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";
import {
  DEFAULT_FIRMWARE_REPOSITORY_URL,
  THUMBNAIL_CACHE_CLEARED_EVENT,
  THUMBNAIL_CACHE_NAME,
} from "../config";
import { clearFirmwareCatalogCache, getRepositoryPreferences } from "../services/firmwareCatalog";
import {
  archiveState,
  loadArchiveCommits,
  parseMirrorRepository,
} from "../services/firmwareArchive";
import ToolDownloadsSection from "../components/ToolDownloadsSection.vue";
import AppUpdateSection from "../components/AppUpdateSection.vue";
import ArchiveSelect from "../components/ArchiveSelect.vue";
import Toast from "../components/Toast.vue";
import type { MFlasherRequestPayload } from "../types/flasher";

const emit = defineEmits<{ openFlasher: [request: MFlasherRequestPayload] }>();

const preferences = reactive({
  firmwareRepositoryUrl: DEFAULT_FIRMWARE_REPOSITORY_URL,
  firmwareArchiveCommit: "",
  firmwareArchiveRepositoryUrl: "",
});
const repositoryUrlDraft = ref("");
const archiveAvailable = computed(() =>
  Boolean(parseMirrorRepository(preferences.firmwareRepositoryUrl)),
);
const selectedArchiveCommit = computed(() =>
  preferences.firmwareArchiveRepositoryUrl === preferences.firmwareRepositoryUrl
    ? preferences.firmwareArchiveCommit
    : "",
);
const settingsLoaded = ref(false);
const preferencesStore = new LazyStore("settings.json", { autoSave: true });
const firmwareFolderDraft = ref("");
const mFlasherInstalled = ref<boolean | null | undefined>(undefined);
const clearingCache = ref(false);
const toast = ref<{ message: string; variant: "success" | "error" } | null>(
  null,
);

onMounted(async () => {
  try {
    if (isTauri()) {
      const saved =
        await preferencesStore.get<Partial<typeof preferences>>("preferences");
      if (saved) Object.assign(preferences, saved);
    } else {
      const saved = localStorage.getItem("fm-mate.preferences");
      if (saved) Object.assign(preferences, JSON.parse(saved));
    }
    if (!preferences.firmwareRepositoryUrl.trim()) {
      preferences.firmwareRepositoryUrl = DEFAULT_FIRMWARE_REPOSITORY_URL;
    }
    repositoryUrlDraft.value = preferences.firmwareRepositoryUrl;
    settingsLoaded.value = true;
    void loadArchiveCommits(preferences.firmwareRepositoryUrl);
    if (isTauri()) {
      try {
        firmwareFolderDraft.value = await invoke<string>(
          "get_firmware_folder_path",
        );
      } catch {
        toast.value = {
          message: "Could not load the firmware folder or tool status.",
          variant: "error",
        };
      }
    }
  } catch {
    toast.value = {
      message: "Could not load saved settings.",
      variant: "error",
    };
  }
});

onActivated(async () => {
  if (!settingsLoaded.value) return;
  try {
    const saved = await getRepositoryPreferences();
    preferences.firmwareArchiveCommit = saved.firmwareArchiveCommit ?? "";
    preferences.firmwareArchiveRepositoryUrl = saved.firmwareArchiveRepositoryUrl ?? "";
  } catch {
    toast.value = { message: "Could not load saved archive date.", variant: "error" };
  }
});

async function savePreferences() {
  if (!settingsLoaded.value) return false;
  toast.value = null;
  try {
    if (isTauri()) {
      await preferencesStore.set("preferences", { ...preferences });
      await preferencesStore.save();
    } else {
      localStorage.setItem("fm-mate.preferences", JSON.stringify(preferences));
    }
    toast.value = { message: "Settings saved.", variant: "success" };
    return true;
  } catch {
    toast.value = { message: "Could not save settings.", variant: "error" };
    return false;
  }
}

async function saveFirmwareFolderPath(path = firmwareFolderDraft.value) {
  if (!settingsLoaded.value || !isTauri()) return;
  toast.value = null;
  try {
    firmwareFolderDraft.value = await invoke<string>(
      "set_firmware_folder_path",
      { path },
    );
    toast.value = {
      message: "Firmware folder saved. Existing files are not moved.",
      variant: "success",
    };
  } catch (error) {
    toast.value = {
      message:
        error instanceof Error
          ? error.message
          : "Could not save firmware folder.",
      variant: "error",
    };
  }
}

function repositoryUrlIsValid(value: string) {
  if (!value.trim()) return true;
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:";
  } catch {
    return false;
  }
}

async function saveRepositoryUrl() {
  if (!repositoryUrlIsValid(repositoryUrlDraft.value)) {
    toast.value = {
      message: "Enter a valid HTTP or HTTPS repository URL.",
      variant: "error",
    };
    return;
  }
  const previous = { ...preferences };
  const url = repositoryUrlDraft.value.trim() || DEFAULT_FIRMWARE_REPOSITORY_URL;
  if (url !== preferences.firmwareRepositoryUrl) {
    preferences.firmwareArchiveCommit = "";
    preferences.firmwareArchiveRepositoryUrl = "";
  }
  preferences.firmwareRepositoryUrl = url;
  if (await savePreferences()) {
    repositoryUrlDraft.value = url;
    void loadArchiveCommits(url);
  } else {
    Object.assign(preferences, previous);
  }
}

async function saveArchiveCommit(commit: string) {
  const previousCommit = preferences.firmwareArchiveCommit;
  const previousUrl = preferences.firmwareArchiveRepositoryUrl;
  preferences.firmwareArchiveCommit = commit;
  preferences.firmwareArchiveRepositoryUrl = commit
    ? preferences.firmwareRepositoryUrl
    : "";
  if (!(await savePreferences())) {
    preferences.firmwareArchiveCommit = previousCommit;
    preferences.firmwareArchiveRepositoryUrl = previousUrl;
  }
}

async function clearCache() {
  if (clearingCache.value) return;
  toast.value = null;
  clearingCache.value = true;
  try {
    await Promise.all([
      clearFirmwareCatalogCache(),
      isTauri()
        ? invoke("clear_thumbnail_cache")
        : "caches" in window
          ? caches.delete(THUMBNAIL_CACHE_NAME)
          : Promise.resolve(),
    ]);
    window.dispatchEvent(new Event(THUMBNAIL_CACHE_CLEARED_EVENT));
    toast.value = { message: "Cache cleared.", variant: "success" };
  } catch (error) {
    toast.value = {
      message:
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : "Could not clear the cache.",
      variant: "error",
    };
  } finally {
    clearingCache.value = false;
  }
}
</script>

<template>
  <section class="view-panel settings-view">
    <div class="view-heading">
      <div>
        <p class="eyebrow">PREFERENCES</p>
        <h1>Settings</h1>
      </div>
    </div>
    <div class="settings-card">
      <form class="settings-group" @submit.prevent="saveFirmwareFolderPath()">
        <h2>FIRMWARE STORAGE</h2>
        <label class="repo-label" for="firmware-folder-path"
          >Firmware folder path</label
        >
        <p class="repo-description">
          Downloaded firmware, saves, and editors are stored here.
        </p>
        <div class="repo-control path-control">
          <input
            id="firmware-folder-path"
            v-model="firmwareFolderDraft"
            type="text"
            placeholder="Absolute folder path"
            :disabled="!settingsLoaded || !isTauri()"
          />
          <button
            class="outline-button"
            type="submit"
            :disabled="!settingsLoaded || !isTauri()"
          >
            SAVE PATH
          </button>
          <button
            class="outline-button"
            type="button"
            :disabled="!settingsLoaded || !isTauri()"
            @click="saveFirmwareFolderPath('')"
          >
            USE DEFAULT
          </button>
        </div>
      </form>
      <form
        class="settings-group repository-form"
        novalidate
        @submit.prevent="saveRepositoryUrl"
      >
        <h2>FIRMWARE REPOSITORY</h2>
        <label class="repo-label" for="firmware-repository-url"
          >Firmware repository URL</label
        >
        <p class="repo-description">
          The address used to check for available firmware releases.
        </p>
        <div class="repo-control">
          <input
            id="firmware-repository-url"
            v-model="repositoryUrlDraft"
            type="url"
            :placeholder="DEFAULT_FIRMWARE_REPOSITORY_URL"
            :disabled="!settingsLoaded"
            :aria-invalid="!repositoryUrlIsValid(repositoryUrlDraft)"
          /><button
            class="outline-button"
            type="submit"
            :disabled="!settingsLoaded"
          >
            SAVE URL
          </button>
          <ArchiveSelect
            v-if="archiveAvailable"
            :selected="selectedArchiveCommit"
            :disabled="!settingsLoaded"
            @change="saveArchiveCommit"
          />
        </div>
        <p v-if="archiveAvailable && archiveState.loading" class="repo-description">
          Loading archive dates…
        </p>
        <div v-if="archiveAvailable && archiveState.error" class="settings-feedback is-error" role="alert">
          Could not load archive dates: {{ archiveState.error }}
          <button type="button" class="outline-button" @click="loadArchiveCommits(preferences.firmwareRepositoryUrl, true)">
            RETRY
          </button>
        </div>
      </form>
      <AppUpdateSection />
      <section class="settings-group">
        <h2>M-BOOTLOADER</h2>
        <p class="repo-description">
          Installing m-bootloader on your FM-1 makes it easier to recover from
          interrupted firmware writes. Connect your device, then use the button
          below to flash it. Do not disconnect or power off the device while
          flashing the bootloader.
        </p>
        <button
          class="action-button action-button--primary"
          type="button"
          :disabled="!isTauri() || mFlasherInstalled !== true"
          @click="emit('openFlasher', { type: 'flash-bootloader' })"
        >
          {{
            !isTauri() || mFlasherInstalled === true
              ? "FLASH M-BOOTLOADER"
              : mFlasherInstalled === false
                ? "M-flasher required, install below!"
                : mFlasherInstalled === null
                  ? "M-FLASHER STATUS UNAVAILABLE"
                  : "CHECKING M-FLASHER…"
          }}
        </button>
        <p v-if="!isTauri()" class="settings-feedback">
          Bootloader flashing is available in the desktop app.
        </p>
      </section>
      <ToolDownloadsSection @m-flasher-status="mFlasherInstalled = $event" />
      <section class="settings-group">
        <h2>CACHE</h2>
        <p class="repo-description">
          Clear cached firmware images and catalog data. Downloaded firmware and
          saved thumbnails are kept.
        </p>
        <button
          class="outline-button"
          type="button"
          :disabled="clearingCache"
          @click="clearCache"
        >
          {{ clearingCache ? "CLEARING…" : "CLEAR CACHE" }}
        </button>
      </section>
    </div>
    <Toast
      v-if="toast"
      :message="toast.message"
      :reset-key="toast"
      :variant="toast.variant"
      @dismiss="toast = null"
    />
  </section>
</template>
