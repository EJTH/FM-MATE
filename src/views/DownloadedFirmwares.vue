<script setup lang="ts">
import { computed, onActivated, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";
import { openUrl } from "@tauri-apps/plugin-opener";
import FirmwareThumbnail from "../components/FirmwareThumbnail.vue";
import NewVersionBanner from "../components/NewVersionBanner.vue";
import FirmwareDetails from "../components/FirmwareDetails.vue";
import InstallDropdown from "../components/InstallDropdown.vue";
import ConfirmationDialog from "../components/ConfirmationDialog.vue";
import type { InstalledFirmware, InstalledVersion } from "../types/firmware";
import {
  firmwareThumbnailUrls,
  getRepositoryUrl,
  isNewerFirmwareVersion,
  loadFirmwareCatalog,
} from "../services/firmwareCatalog";
import type { FirmwareRecord } from "../services/firmwareCatalog";
import type { MFlasherRequestPayload } from "../types/flasher";
import { finishEmulatorSession } from "../services/emulatorSession";
import type { EmulatorSavePrompt } from "../services/emulatorSession";
import FavoriteButton from "../components/FavoriteButton.vue";

const props = defineProps<{ openFirmwareId?: string | null }>();
const emit = defineEmits<{
  openSaves: [filter: { firmwareId: string; version: string }];
  openFlasher: [request: MFlasherRequestPayload];
  detailsOpened: [firmwareId: string];
}>();

interface ScannedFirmware {
  id: string;
  name: string;
  version: string;
  kind: string;
  description: string;
  author: string;
  fileName: string;
  path: string;
  downloadedAt: string;
  savesPath: string;
  savesCount: number;
  saves: string[];
  currentVersion?: string | null;
  thumbnailData?: string | null;
  info: Record<string, unknown>;
  editorPath?: string | null;
}

const store = new LazyStore("settings.json", { autoSave: true });
const scannedFirmwares = ref<ScannedFirmware[]>([]);
const favoriteIds = ref<string[]>([]);
const catalogFirmwares = ref<FirmwareRecord[]>([]);
const catalogRepositoryUrl = ref("");
const catalogNotice = ref("");
const updateDownloadingId = ref("");
const updateMessage = ref("");
const emulating = ref(false);
const loading = ref(true);
const loadError = ref(false);
const folderError = ref("");
const openingFolder = ref(false);
const selectedFirmware = ref<InstalledFirmware | null>(null);
const actionMessage = ref("");
const pendingVersionDelete = ref<{
  firmware: InstalledFirmware;
  version: InstalledVersion;
} | null>(null);

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

async function loadDownloads() {
  loading.value = true;
  loadError.value = false;
  actionMessage.value = "";
  try {
    if (isTauri()) {
      favoriteIds.value =
        (await store.get<string[]>("favoriteFirmwareIds")) ?? [];
      scannedFirmwares.value = await invoke<ScannedFirmware[]>(
        "scan_installed_firmwares",
      );
      await store.set("downloadedFirmwares", scannedFirmwares.value);
      await store.save();
    } else {
      favoriteIds.value = JSON.parse(
        localStorage.getItem("fm-mate.favoriteFirmwareIds") ?? "[]",
      ) as string[];
      scannedFirmwares.value = JSON.parse(
        localStorage.getItem("fm-mate.downloadedFirmwares") ?? "[]",
      ) as ScannedFirmware[];
    }
  } catch {
    loadError.value = true;
  } finally {
    loading.value = false;
  }
}

async function activateDownloads() {
  await loadDownloads();
  const requestedId = props.openFirmwareId;
  if (!requestedId) return;
  const match = firmwares.value.find((firmware) => firmware.id === requestedId);
  if (match) selectedFirmware.value = match;
  else actionMessage.value = "That firmware is no longer installed.";
  emit("detailsOpened", requestedId);
}

onActivated(activateDownloads);

async function loadCatalog() {
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
    catalogNotice.value = "Could not check for firmware updates.";
  }
}

onActivated(loadCatalog);

function availableUpdate(firmware: InstalledFirmware) {
  const latest = catalogFirmwares.value
    .filter(
      (item) =>
        item.id === firmware.id &&
        typeof item.version === "string" &&
        item.version.trim(),
    )
    .sort((a, b) =>
      (b.version ?? "").localeCompare(a.version ?? "", undefined, {
        numeric: true,
        sensitivity: "base",
      }),
    )[0];
  if (!latest?.version) return null;
  const newestInstalled = firmware.installedVersions
    .map((item) => item.version)
    .sort((a, b) =>
      b.localeCompare(a, undefined, { numeric: true, sensitivity: "base" }),
    )[0];
  if (
    !newestInstalled ||
    !isNewerFirmwareVersion(latest.version, newestInstalled)
  )
    return null;
  return latest;
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
    actionMessage.value = "Could not save this favorite.";
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
  folderError.value = "";
  if (!isTauri()) {
    folderError.value =
      "Opening the firmware folder is available in the desktop app.";
    return;
  }
  openingFolder.value = true;
  try {
    await invoke("open_firmware_folder");
  } catch (error) {
    folderError.value =
      error instanceof Error
        ? `Could not open the firmware folder: ${error.message}`
        : "Could not open the firmware folder.";
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
    actionMessage.value =
      error instanceof Error ? error.message : "Could not open the editor.";
  }
}

async function openInEmulator(
  firmware: InstalledFirmware,
  version: InstalledVersion,
) {
  if (emulating.value) return;
  updateMessage.value = "";
  if (!isTauri()) {
    updateMessage.value = "Open in emulator is available in the desktop app.";
    return;
  }
  emulating.value = true;
  function report(message: string) {
    if (selectedFirmware.value?.id === firmware.id)
      updateMessage.value = message;
    else actionMessage.value = message;
  }
  try {
    const savePrompt = await invoke<EmulatorSavePrompt>(
      "open_firmware_in_emulator",
      { id: firmware.id, version: version.version },
    );
    const saved = await finishEmulatorSession(savePrompt);
    if (saved) {
      await loadDownloads();
      if (selectedFirmware.value?.id === firmware.id) {
        selectedFirmware.value =
          firmwares.value.find((item) => item.id === firmware.id) ?? null;
      }
      report(
        `Emulator session saved as ${saved.fileName}.${saved.warning ? ` ${saved.warning}` : ""}`,
      );
    } else {
      report("Emulator session not saved.");
    }
  } catch (error) {
    report(error instanceof Error ? error.message : String(error));
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
  updateMessage.value = "";
  try {
    const packageUrl = new URL(
      update.pkg,
      `${new URL(catalogRepositoryUrl.value).origin}/`,
    );
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
    await loadDownloads();
    selectedFirmware.value =
      firmwares.value.find((item) => item.id === firmware.id) ?? null;
    updateMessage.value = `${firmware.name} ${update.version} downloaded.`;
  } catch (error) {
    updateMessage.value =
      error instanceof Error ? error.message : "Firmware download failed.";
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
    updateMessage.value = "Could not open the release notes.";
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
    await loadDownloads();
    selectedFirmware.value =
      firmwares.value.find((item) => item.id === pending.firmware.id) ?? null;
    actionMessage.value = `Deleted ${pending.firmware.name} ${pending.version.version}.`;
  } catch (error) {
    actionMessage.value =
      error instanceof Error
        ? error.message
        : "Could not delete this firmware version.";
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
  <section class="view-panel">
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
    <p v-if="folderError" class="folder-error" role="status">
      {{ folderError }}
    </p>
    <p v-if="actionMessage" class="action-message" role="status">
      {{ actionMessage }}
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
    <div v-else class="installed-grid">
      <article
        v-for="firmware in sortedFirmwares"
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
      :update-message="updateMessage"
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
  </section>
</template>

<style>
.downloaded-heading-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.folder-error,
.action-message {
  margin: -12px 0 14px;
  color: #ed9a7b;
  font:
    12px "Share Tech Mono",
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
