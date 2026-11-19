<script setup lang="ts">
import { computed, onActivated, ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";
import { openUrl } from "@tauri-apps/plugin-opener";
import FirmwareThumbnail from "../components/FirmwareThumbnail.vue";
import NewVersionBanner from "../components/NewVersionBanner.vue";
import FirmwareDetails from "../components/FirmwareDetails.vue";
import InstallDropdown from "../components/InstallDropdown.vue";
import ConfirmationDialog from "../components/ConfirmationDialog.vue";
import Toast from "../components/Toast.vue";
import type { InstalledFirmware, InstalledVersion } from "../types/firmware";
import {
  firmwareThumbnailUrls,
  getRepositoryUrl,
  loadFirmwareCatalog,
  findFirmwareUpdate,
  getFavoriteFirmwareIds,
  getInstalledFirmwares,
} from "../services/firmwareCatalog";
import type { FirmwareRecord, ScannedFirmware } from "../services/firmwareCatalog";
import type { MFlasherRequestPayload } from "../types/flasher";
import { finishEmulatorSession } from "../services/emulatorSession";
import type { EmulatorSavePrompt } from "../services/emulatorSession";
import FavoriteButton from "../components/FavoriteButton.vue";

const props = defineProps<{
  openFirmwareId?: string | null;
  showUpdatesRequest?: number | null;
}>();
const emit = defineEmits<{
  openSaves: [filter: { firmwareId: string; version: string }];
  openFlasher: [request: MFlasherRequestPayload];
  detailsOpened: [firmwareId: string];
  updatesFilterApplied: [requestId: number];
}>();


const store = new LazyStore("settings.json", { autoSave: true });
const scannedFirmwares = ref<ScannedFirmware[]>([]);
const favoriteIds = ref<string[]>([]);
const catalogFirmwares = ref<FirmwareRecord[]>([]);
const catalogRepositoryUrl = ref("");
const catalogNotice = ref("");
const catalogLoading = ref(true);
const catalogFailed = ref(false);
const updateDownloadingId = ref("");
const emulating = ref(false);
const loading = ref(true);
const loadError = ref(false);
const openingFolder = ref(false);
const selectedFirmware = ref<InstalledFirmware | null>(null);
const updatesOnly = ref(false);
const toast = ref<{
  message: string;
  variant: "success" | "error" | "info";
} | null>(null);
const pendingVersionDelete = ref<{
  firmware: InstalledFirmware;
  version: InstalledVersion;
} | null>(null);
watch(
  () => props.showUpdatesRequest,
  (requestId) => {
    if (requestId == null) return;
    updatesOnly.value = true;
    selectedFirmware.value = null;
    pendingVersionDelete.value = null;
    emit("updatesFilterApplied", requestId);
  },
  { immediate: true },
);

const firmwares = computed<InstalledFirmware[]>(() => {
  const grouped = new Map<string, InstalledFirmware>();
  for (const item of scannedFirmwares.value) {
    let firmware = grouped.get(item.id);
    if (!firmware) {
      firmware = {
        id: item.id,
        name: item.name,
        kind: item.kind,
        description: item.description,
        author: item.author,
        currentVersion: item.currentVersion,
        thumbnailData: item.thumbnailData,
        info: item.info,
        installedVersions: [],
        savesCount: 0,
      };
      grouped.set(item.id, firmware);
    } else if (!firmware.thumbnailData && item.thumbnailData) {
      firmware.thumbnailData = item.thumbnailData;
    }
    firmware.installedVersions.push({
      version: item.version,
      editorPath: item.editorPath,
      path: item.path,
      savesPath: item.savesPath,
      savesCount: item.savesCount,
      saves: item.saves,
      downloadedAt: item.downloadedAt,
    });
    firmware.savesCount += item.savesCount;
  }

  return [...grouped.values()].map((firmware) => ({
    ...firmware,
    installedVersions: firmware.installedVersions.sort((a, b) =>
      b.version.localeCompare(a.version, undefined, {
        numeric: true,
        sensitivity: "base",
      }),
    ),
  }));
});

const sortedFirmwares = computed(() =>
  [...firmwares.value].sort((a, b) => {
    const favoriteOrder =
      Number(favoriteIds.value.includes(b.id)) -
      Number(favoriteIds.value.includes(a.id));
    return favoriteOrder || a.name.localeCompare(b.name);
  }),
);
const visibleFirmwares = computed(() =>
  updatesOnly.value
    ? sortedFirmwares.value.filter((firmware) => availableUpdate(firmware))
    : sortedFirmwares.value,
);

async function loadDownloads() {
  loading.value = true;
  loadError.value = false;
  try {
    const [firmwares, ids] = await Promise.all([
      getInstalledFirmwares(),
      getFavoriteFirmwareIds(),
    ]);
    scannedFirmwares.value = firmwares;
    favoriteIds.value = ids;
    return true;
  } catch {
    loadError.value = true;
    return false;
  } finally {
    loading.value = false;
  }
}

async function activateDownloads() {
  const loaded = await loadDownloads();
  const requestedId = props.openFirmwareId;
  if (!requestedId) return;
  if (loaded) {
    const match = firmwares.value.find((firmware) => firmware.id === requestedId);
    if (match) selectedFirmware.value = match;
    else
      toast.value = {
        message: "That firmware is no longer installed.",
        variant: "info",
      };
  }
  emit("detailsOpened", requestedId);
}

onActivated(activateDownloads);

async function loadCatalog() {
  catalogLoading.value = true;
  catalogFailed.value = false;
  catalogRepositoryUrl.value = "";
  try {
    catalogRepositoryUrl.value = await getRepositoryUrl();
    const catalog = await loadFirmwareCatalog();
    catalogFirmwares.value = catalog.firmwares;
    catalogRepositoryUrl.value = catalog.repositoryUrl;
    catalogNotice.value = catalog.stale
      ? "Repository refresh failed; update information may be out of date."
      : "";
  } catch {
    catalogFirmwares.value = [];
    catalogFailed.value = true;
    catalogNotice.value = "Could not check for firmware updates.";
  } finally {
    catalogLoading.value = false;
  }
}

onActivated(loadCatalog);

function availableUpdate(firmware: InstalledFirmware) {
  return findFirmwareUpdate(
    firmware.id,
    catalogFirmwares.value,
    scannedFirmwares.value,
  );
}

async function toggleFavorite(id: string) {
  const previous = favoriteIds.value;
  const next = previous.includes(id)
    ? previous.filter((favoriteId) => favoriteId !== id)
    : [...previous, id];
  favoriteIds.value = next;
  try {
    if (isTauri()) {
      await store.set("favoriteFirmwareIds", next);
      await store.save();
    } else {
      localStorage.setItem("fm-mate.favoriteFirmwareIds", JSON.stringify(next));
    }
  } catch {
    favoriteIds.value = previous;
    toast.value = { message: "Could not save this favorite.", variant: "error" };
  }
}

function editorUrl(firmware: InstalledFirmware) {
  const value = firmware.info.editor;
  if (typeof value !== "string" || !value.trim()) return "";
  try {
    const url = new URL(value);
    return ["https:", "http:"].includes(url.protocol) ? url.toString() : "";
  } catch {
    return "";
  }
}

function hasEditor(firmware: InstalledFirmware) {
  return Boolean(editorUrl(firmware));
}

async function openFirmwareFolder() {
  toast.value = null;
  if (!isTauri()) {
    toast.value = {
      message: "Opening the firmware folder is available in the desktop app.",
      variant: "info",
    };
    return;
  }
  openingFolder.value = true;
  try {
    await invoke("open_firmware_folder");
  } catch (error) {
    toast.value = {
      message:
        error instanceof Error
          ? `Could not open the firmware folder: ${error.message}`
          : "Could not open the firmware folder.",
      variant: "error",
    };
  } finally {
    openingFolder.value = false;
  }
}

async function openEditor(
  firmware: InstalledFirmware,
  version?: InstalledVersion,
) {
  const targetVersion =
    version ??
    firmware.installedVersions.find(
      (item) => item.version === firmware.currentVersion,
    ) ??
    firmware.installedVersions[0];
  try {
    if (isTauri() && targetVersion?.editorPath) {
      const localEditorUrl = await invoke<string>("serve_firmware_editor", {
        id: firmware.id,
        version: targetVersion.version,
      });
      const openedInChrome = await invoke<boolean>("launch_editor_in_chrome", {
        url: localEditorUrl,
      });
      if (!openedInChrome) await openUrl(localEditorUrl);
      return;
    }
    const url = editorUrl(firmware);
    if (!url) return;
    if (isTauri()) await openUrl(url);
    else window.open(url, "_blank", "noopener,noreferrer");
  } catch (error) {
    toast.value = {
      message:
        error instanceof Error ? error.message : "Could not open the editor.",
      variant: "error",
    };
  }
}

async function openInEmulator(
  firmware: InstalledFirmware,
  version: InstalledVersion,
) {
  if (emulating.value) return;
  toast.value = null;
  if (!isTauri()) {
    toast.value = {
      message: "Open in emulator is available in the desktop app.",
      variant: "info",
    };
    return;
  }
  emulating.value = true;
  toast.value = {
    message: `Starting ${firmware.name} ${version.version} in emulator...`,
    variant: "info",
  };
  try {
    const savePrompt = await invoke<EmulatorSavePrompt>(
      "open_firmware_in_emulator",
      { id: firmware.id, version: version.version },
    );
    const saved = await finishEmulatorSession(savePrompt);
    if (saved) {
      if (!(await loadDownloads())) {
        throw new Error(
          `Emulator session saved as ${saved.fileName}, but the library could not be refreshed.${saved.warning ? ` ${saved.warning}` : ""}`,
        );
      }
      if (selectedFirmware.value?.id === firmware.id) {
        selectedFirmware.value =
          firmwares.value.find((item) => item.id === firmware.id) ?? null;
      }
      toast.value = {
        message: `Emulator session saved as ${saved.fileName}.${saved.warning ? ` ${saved.warning}` : ""}`,
        variant: "success",
      };
    } else {
      toast.value = { message: "Emulator session not saved.", variant: "info" };
    }
  } catch (error) {
    toast.value = {
      message: error instanceof Error ? error.message : String(error),
      variant: "error",
    };
  } finally {
    emulating.value = false;
  }
}

function openSaves(filter: { firmwareId: string; version: string }) {
  emit("openSaves", filter);
}

async function downloadUpdate(firmware: InstalledFirmware) {
  const update = availableUpdate(firmware);
  if (!update?.pkg || updateDownloadingId.value) return;
  updateDownloadingId.value = firmware.id;
  toast.value = null;
  try {
    const packageUrl = new URL(update.pkg, catalogRepositoryUrl.value);
    if (!["https:", "http:"].includes(packageUrl.protocol))
      throw new Error("The firmware package URL must use HTTP or HTTPS.");
    const fileName = decodeURIComponent(
      packageUrl.pathname.split("/").pop() ?? "",
    );
    if (!fileName)
      throw new Error(
        "The repository entry does not include a package file name.",
      );
    const downloadedAt = new Date().toISOString();
    const thumbnailUrl = firmwareThumbnailUrls(
      update.id,
      update.card,
      catalogRepositoryUrl.value,
    )[0];
    if (isTauri()) {
      await invoke<string>("download_firmware", {
        url: packageUrl.toString(),
        id: update.id,
        version: update.version || "unknown",
        thumbnailUrl,
        firmwareInfo: { ...update, fileName: "firmware.fwsc", downloadedAt },
      });
    } else {
      const response = await fetch(packageUrl);
      if (!response.ok)
        throw new Error(`Firmware download failed (${response.status}).`);
      const objectUrl = URL.createObjectURL(await response.blob());
      const anchor = document.createElement("a");
      anchor.href = objectUrl;
      anchor.download = fileName;
      anchor.click();
      URL.revokeObjectURL(objectUrl);
    }
    if (!(await loadDownloads())) {
      throw new Error(
        "Firmware downloaded, but the library could not be refreshed.",
      );
    }
    selectedFirmware.value =
      firmwares.value.find((item) => item.id === firmware.id) ?? null;
    toast.value = {
      message: `${firmware.name} ${update.version} downloaded.`,
      variant: "success",
    };
  } catch (error) {
    toast.value = {
      message:
        error instanceof Error ? error.message : "Firmware download failed.",
      variant: "error",
    };
  } finally {
    updateDownloadingId.value = "";
  }
}

async function openReleaseNotes(firmware: InstalledFirmware) {
  const update = availableUpdate(firmware);
  if (!update?.release) return;
  try {
    const releaseUrl = new URL(update.release);
    if (!["https:", "http:"].includes(releaseUrl.protocol)) return;
    if (isTauri()) await openUrl(releaseUrl.toString());
    else window.open(releaseUrl.toString(), "_blank", "noopener,noreferrer");
  } catch {
    toast.value = {
      message: "Could not open the release notes.",
      variant: "error",
    };
  }
}

function deleteVersion(firmware: InstalledFirmware, version: InstalledVersion) {
  pendingVersionDelete.value = { firmware, version };
}

async function confirmDeleteVersion() {
  const pending = pendingVersionDelete.value;
  if (!pending) return;
  pendingVersionDelete.value = null;
  try {
    await invoke("delete_firmware_version", {
      id: pending.firmware.id,
      version: pending.version.version,
    });
    if (!(await loadDownloads())) {
      throw new Error(
        "Firmware deleted, but the library could not be refreshed.",
      );
    }
    selectedFirmware.value =
      firmwares.value.find((item) => item.id === pending.firmware.id) ?? null;
    toast.value = {
      message: `Deleted ${pending.firmware.name} ${pending.version.version}.`,
      variant: "success",
    };
  } catch (error) {
    toast.value = {
      message:
        error instanceof Error
          ? error.message
          : "Could not delete this firmware version.",
      variant: "error",
    };
  }
}

async function installFirmware(firmware: InstalledFirmware, version: string) {
  let automaticallyBackup = true;
  try {
    const preferences = isTauri()
      ? await store.get<{ automaticallyBackupBeforeSwitching?: boolean }>(
          "preferences",
        )
      : (JSON.parse(localStorage.getItem("fm-mate.preferences") ?? "{}") as {
          automaticallyBackupBeforeSwitching?: boolean;
        });
    automaticallyBackup =
      preferences?.automaticallyBackupBeforeSwitching ?? true;
  } catch {
    // Keep the application's default preference if settings could not be read.
  }
  emit("openFlasher", {
    type: "switch-firmware",
    firmwareId: firmware.id,
    firmwareName: firmware.name,
    version,
    automaticallyBackup,
  });
}
</script>

<template>
  <section id="library" class="view-panel">
    <div class="view-heading">
      <div>
        <p class="eyebrow">LOCAL LIBRARY</p>
        <h1>Downloaded Firmwares</h1>
      </div>
      <div class="downloaded-heading-actions">
        <button
          class="outline-button"
          type="button"
          :disabled="openingFolder"
          @click="openFirmwareFolder"
        >
          {{ openingFolder ? "OPENING…" : "FIRMWARE FOLDER ↗" }}
        </button>
        <button
          class="outline-button"
          type="button"
          :disabled="loading"
          @click="loadDownloads"
        >
          REFRESH
        </button>
      </div>
    </div>
    <div class="library-filters">
      <label class="library-updates-only">
        <input v-model="updatesOnly" type="checkbox" />
        Only show pending updates
      </label>
    </div>
    <p
      v-if="updatesOnly && catalogNotice && !catalogFailed"
      class="library-notice"
      role="status"
    >
      {{ catalogNotice }}
    </p>
    <div v-if="loading" class="library-state">
      <h2>Scanning firmware folder…</h2>
    </div>
    <div v-else-if="loadError" class="library-state">
      <h2>Could not read installed firmwares</h2>
      <p>Try refreshing the firmware folder scan.</p>
    </div>
    <div v-else-if="!firmwares.length" class="library-state">
      <span class="empty-mark">↓</span>
      <h2>No firmware installed</h2>
      <p>Downloaded firmware packages will appear here.</p>
    </div>
    <div v-else-if="updatesOnly && catalogLoading" class="library-state">
      <h2>Checking for firmware updates…</h2>
    </div>
    <div v-else-if="updatesOnly && catalogFailed" class="library-state">
      <h2>Could not check for firmware updates</h2>
      <p>Return to this tab to retry.</p>
    </div>
    <div
      v-else-if="updatesOnly && !visibleFirmwares.length"
      class="library-state"
    >
      <h2>No firmware updates available</h2>
      <p>Downloaded firmware with updates will appear here.</p>
    </div>
    <div v-else class="installed-grid">
      <article
        v-for="firmware in visibleFirmwares"
        :key="firmware.id"
        class="installed-card"
        tabindex="0"
        :aria-label="`Open details for ${firmware.name}${availableUpdate(firmware) ? ' - New version!' : ''}`"
        @click="selectedFirmware = firmware"
        @keydown.enter.self="selectedFirmware = firmware"
        @keydown.space.prevent.self="selectedFirmware = firmware"
      >
        <div class="installed-card__thumbnail">
          <img
            v-if="firmware.thumbnailData && !availableUpdate(firmware)"
            :src="firmware.thumbnailData"
            :alt="`${firmware.name} thumbnail`"
          />
          <FirmwareThumbnail
            v-else
            :id="firmware.id"
            :card="availableUpdate(firmware)?.card"
            :version="
              availableUpdate(firmware)?.version ||
              firmware.installedVersions[0]?.version
            "
            :fallback-data="firmware.thumbnailData"
            :repository-url="catalogRepositoryUrl"
            :alt="`${firmware.name} thumbnail`"
          />
          <NewVersionBanner v-if="availableUpdate(firmware)" />
        </div>
        <h2 class="installed-card__name">{{ firmware.name }}</h2>
        <p class="installed-card__description">
          {{ firmware.description || "No description provided." }}
        </p>
        <div class="installed-card__versions">
          <strong>INSTALLED VERSIONS</strong>
          <div>
            <span
              v-for="version in firmware.installedVersions"
              :key="version.version"
              class="installed-version"
              >{{ version.version
              }}{{
                firmware.currentVersion === version.version ? " · CURRENT" : ""
              }}</span
            >
          </div>
        </div>
        <div class="installed-card__buttons">
          <FavoriteButton
            :favorited="favoriteIds.includes(firmware.id)"
            :label="firmware.name"
            @update:favorited="toggleFavorite(firmware.id)"
          />
          <button
            class="outline-button"
            type="button"
            :disabled="!hasEditor(firmware)"
            @click.stop="openEditor(firmware)"
          >
            OPEN EDITOR
          </button>
          <InstallDropdown
            :versions="firmware.installedVersions.map((item) => item.version)"
            @select="installFirmware(firmware, $event)"
          />
        </div>
      </article>
    </div>

    <FirmwareDetails
      v-if="selectedFirmware"
      :firmware="selectedFirmware"
      :repository-url="catalogRepositoryUrl"
      :update="availableUpdate(selectedFirmware)"
      :update-downloading="updateDownloadingId === selectedFirmware.id"
      :emulating="emulating"
      :catalog-notice="catalogNotice"
      @close="selectedFirmware = null"
      @open-in-emulator="openInEmulator(selectedFirmware, $event)"
      @open-editor="openEditor(selectedFirmware, $event)"
      @delete-version="deleteVersion(selectedFirmware, $event)"
      @open-saves="openSaves"
      @install-version="installFirmware(selectedFirmware, $event)"
      @download-update="downloadUpdate(selectedFirmware)"
      @open-release="openReleaseNotes(selectedFirmware)"
    />
    <ConfirmationDialog
      v-if="pendingVersionDelete"
      title="DELETE FIRMWARE VERSION?"
      :message="`Delete ${pendingVersionDelete.firmware.name} ${pendingVersionDelete.version.version}? This also deletes its saves and downloaded editor.`"
      confirm-label="DELETE VERSION"
      confirm-variant="danger"
      @confirm="confirmDeleteVersion"
      @secondary="pendingVersionDelete = null"
      @cancel="pendingVersionDelete = null"
    />
    <Toast
      v-if="toast"
      :message="toast.message"
      :variant="toast.variant"
      :reset-key="toast"
      @dismiss="toast = null"
    />
  </section>
</template>

<style>
.downloaded-heading-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.library-filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;
}
.library-filters .library-updates-only {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: #b49bdc;
  font-size: 13px;
}
.library-updates-only input {
  accent-color: #65e1c1;
}
.library-notice {
  color: #b9b0a7;
  font:
    12px/1.5 "Share Tech Mono",
    monospace;
}

.library-state {
  min-height: 42vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 25px;
  text-align: center;
  background: rgb(40 37 36 / 70%);
  border: 1px solid #48413f;
}

.library-state h2 {
  margin: 0 0 5px;
  color: #e3dcd2;
  font-size: 21px;
  font-weight: 600;
}

.library-state p {
  margin: 0;
  color: #a49b97;
  font:
    13px "Share Tech Mono",
    monospace;
}

.installed-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 16px;
}

.installed-card {
  display: flex;
  min-width: 0;
  flex-direction: column;
  padding: 0 14px 14px;
  background: rgb(40 37 36 / 78%);
  border: 1px solid #48413f;
  cursor: pointer;
  transition:
    border-color 0.15s,
    background 0.15s;
}

.installed-card:hover,
.installed-card:focus-visible {
  background: rgb(48 44 43 / 90%);
  border-color: #766382;
  outline: none;
}

.installed-card__thumbnail {
  position: relative;
  display: grid;
  width: calc(100% + 28px);
  aspect-ratio: 1 / 1;
  margin: 0 -14px;
  place-items: center;
  overflow: hidden;
  background: #211f1e;
}

.installed-card__thumbnail > img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.installed-card__thumbnail .firmware-thumbnail {
  width: 100%;
  height: 100%;
  flex-basis: auto;
  border: 0;
}

.installed-card__name {
  margin: 13px 0 4px;
  color: #e8e1d7;
  font-size: 21px;
  font-weight: 600;
}

.installed-card__description {
  display: -webkit-box;
  min-height: 3.9em;
  margin: 0;
  overflow: hidden;
  color: #b9b0a7;
  font:
    12px/1.5 "Share Tech Mono",
    monospace;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
}

.installed-card__versions {
  margin-top: 13px;
  padding-top: 10px;
  border-top: 1px solid #514754;
}

.installed-card__versions > strong {
  display: block;
  margin-bottom: 7px;
  color: #aa97c2;
  font-size: 10px;
  letter-spacing: 0.13em;
}

.installed-card__versions > div {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.installed-version {
  padding: 3px 6px;
  color: #77dfc0;
  background: #1f3531;
  border: 1px solid #3c766a;
  font:
    11px "Share Tech Mono",
    monospace;
}

.installed-card__buttons {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr) minmax(0, 1fr);
  gap: 7px;
  margin-top: auto;
  padding-top: 15px;
}

.installed-card__buttons .outline-button {
  min-width: 0;
  padding-inline: 5px;
  font-size: 10px;
}

.favorite-button {
  display: grid;
  min-width: 0;
  place-items: center;
  padding: 0;
  color: #a99d9c;
  background: #302d2c;
  border: 1px solid #514b4a;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}

.favorite-button:hover,
.favorite-button.is-favorite {
  color: #ff807f;
  border-color: #9a5559;
}

.favorite-button.is-favorite {
  background: #3d282a;
  text-shadow: 0 0 8px rgb(255 105 115 / 35%);
}

.installed-card__buttons .outline-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

@media (max-width: 600px) {
  .downloaded-heading-actions {
    width: 100%;
  }

  .installed-grid {
    grid-template-columns: 1fr;
  }
}
</style>
