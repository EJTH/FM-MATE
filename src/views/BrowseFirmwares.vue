<script setup lang="ts">
import { computed, onDeactivated, onActivated, ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";
import DetailedResultCard from "../components/DetailedResultCard.vue";
import FirmwareResultCard from "../components/FirmwareResultCard.vue";
import ConfirmationDialog from "../components/ConfirmationDialog.vue";
import ArchiveSelect from "../components/ArchiveSelect.vue";
import Toast from "../components/Toast.vue";
import {
  firmwareThumbnailUrls,
  getRepositoryPreferences,
  isNewerFirmwareVersion,
  loadFirmwareCatalog,
  saveFirmwareArchiveCommit,
  DownloadedFirmware,
} from "../services/firmwareCatalog";
import { DEFAULT_FIRMWARE_REPOSITORY_URL } from "../config";
import { archiveState, loadArchiveCommits, parseMirrorRepository } from "../services/firmwareArchive";
import type {
  FirmwareCatalogLicense,
  FirmwareRecord,
} from "../services/firmwareCatalog";
import type { MFlasherRequestPayload } from "../types/flasher";
import { finishEmulatorSession } from "../services/emulatorSession";
import type { EmulatorSavePrompt } from "../services/emulatorSession";

type DeviceFilter = "all" | "fm1" | "smk37";

const store = new LazyStore("settings.json", { autoSave: true });
const firmwares = ref<FirmwareRecord[]>([]);
const updated = ref("");
const repositoryUrl = ref("");
const configuredRepositoryUrl = ref("");
const selectedArchiveCommit = ref("");
const savingArchive = ref(false);
const archiveAvailable = computed(() =>
  Boolean(parseMirrorRepository(configuredRepositoryUrl.value)),
);
const repositoryLicense = ref<FirmwareCatalogLicense | null | undefined>();
const search = ref("");
const deviceFilter = ref<DeviceFilter>("all");
const selectedKinds = ref<string[]>([]);
const sortBy = ref<"name" | "kind" | "added" | "date" | "stars" | "installs">(
  "installs",
);
const sortDescending = ref(true);
const loading = ref(false);
const errorMessage = ref("");
const resultView = ref<"detailed" | "grid">("grid");
const selectedResult = ref<FirmwareRecord | null>(null);
const tryingId = ref("");
const firmwareKeepPrompt = ref<FirmwareRecord | null>(null);
const controller = ref<AbortController | null>(null);
const emit = defineEmits<{
  "open-settings": [];
  "open-installed-firmware": [firmwareId: string];
  openFlasher: [request: MFlasherRequestPayload];
}>();
const downloadedFirmwares = ref<DownloadedFirmware[]>([]);
const downloadingId = ref("");
const toast = ref<{
  message: string;
  variant: "success" | "error" | "info";
} | null>(null);
const catalogSource = computed(() => {
  try {
    const url = new URL(repositoryUrl.value);
    return ["https:", "http:"].includes(url.protocol)
      ? { href: `${url.origin}/`, label: url.host }
      : null;
  } catch {
    return null;
  }
});
const repositoryLicenseUrl = computed(() => {
  const value = repositoryLicense.value?.url;
  if (!value) return "";
  try {
    const url = new URL(value);
    return ["https:", "http:"].includes(url.protocol) ? url.toString() : "";
  } catch {
    return "";
  }
});

const repositoryLicenseDisplay = computed(() => {
  if (!repositoryLicense.value) return "";

  // Prefer explicit attribution text if available
  const attribution = repositoryLicense.value.attribution?.trim();
  if (attribution) return attribution;

  // Fall back to license name if available
  const license = repositoryLicense.value.license?.trim();
  if (license) return `Licensed under ${license}`;

  // If we have a URL but no text, provide generic attribution
  if (repositoryLicenseUrl.value) return "View license";

  return "";
});

const hasRepositoryLicense = computed(() => !!repositoryLicenseDisplay.value);

watch(sortBy, (field) => {
  sortDescending.value = ["added", "date", "stars", "installs"].includes(field);
});

watch(resultView, async (view) => {
  if (isTauri()) {
    await store.set("resultViewLayout", view);
    await store.save();
  } else {
    localStorage.setItem("fm-mate.resultViewLayout", view);
  }
});

async function loadRepository() {
  controller.value?.abort();
  const request = new AbortController();
  controller.value = request;
  loading.value = true;
  errorMessage.value = "";

  try {
    const catalog = await loadFirmwareCatalog(request.signal);

    const previousKinds = new Set(availableKinds.value);
    const previousSelection = new Set(selectedKinds.value);
    firmwares.value = catalog.firmwares;
    selectedKinds.value = availableKinds.value.filter(
      (kind) => !previousKinds.has(kind) || previousSelection.has(kind),
    );
    updated.value = catalog.updated;
    repositoryUrl.value = catalog.repositoryUrl;
    repositoryLicense.value = catalog.license;
    if (catalog.stale)
      errorMessage.value =
        "Could not refresh the repository; showing the cached firmware list.";
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") return;
    errorMessage.value =
      error instanceof Error
        ? error.message
        : "Could not load the firmware repository.";
  } finally {
    if (controller.value === request) loading.value = false;
  }
}

onActivated(loadRepository);
onDeactivated(() => controller.value?.abort());

async function loadArchivePreference() {
  try {
    const preferences = await getRepositoryPreferences();
    const url = preferences.firmwareRepositoryUrl?.trim() || DEFAULT_FIRMWARE_REPOSITORY_URL;
    configuredRepositoryUrl.value = url;
    selectedArchiveCommit.value = preferences.firmwareArchiveRepositoryUrl === url
      ? preferences.firmwareArchiveCommit ?? ""
      : "";
    void loadArchiveCommits(url);
  } catch {
    toast.value = { message: "Could not load the saved archive date.", variant: "error" };
  }
}

async function changeArchiveCommit(commit: string) {
  if (savingArchive.value) return;
  savingArchive.value = true;
  try {
    await saveFirmwareArchiveCommit(configuredRepositoryUrl.value, commit);
    selectedArchiveCommit.value = commit;
    selectedResult.value = null;
    await loadRepository();
  } catch (error) {
    toast.value = {
      message: error instanceof Error ? error.message : "Could not save the archive date.",
      variant: "error",
    };
  } finally {
    savingArchive.value = false;
  }
}

onActivated(loadArchivePreference);

async function loadDownloadedFirmwares() {
  try {
    if (isTauri()) {
      downloadedFirmwares.value = await invoke<DownloadedFirmware[]>(
        "scan_installed_firmwares",
      );
      await store.set("downloadedFirmwares", downloadedFirmwares.value);
      await store.save();
    } else {
      downloadedFirmwares.value = JSON.parse(
        localStorage.getItem("fm-mate.downloadedFirmwares") ?? "[]",
      ) as DownloadedFirmware[];
    }
  } catch {
    downloadedFirmwares.value = [];
  }
}

onActivated(loadDownloadedFirmwares);

async function loadViewPreference() {
  try {
    if (isTauri()) {
      const savedView = await store.get<"detailed" | "grid">(
        "resultViewLayout",
      );
      if (savedView === "detailed" || savedView === "grid") {
        resultView.value = savedView;
      }
    } else {
      const savedView = localStorage.getItem("fm-mate.resultViewLayout");
      if (savedView === "detailed" || savedView === "grid") {
        resultView.value = savedView;
      }
    }
  } catch {
    // Use default if loading preference fails
  }
}

onActivated(loadViewPreference);

function hasDownloaded(firmware: FirmwareRecord) {
  return downloadedFirmwares.value.some(
    (download) =>
      download.id === firmware.id &&
      download.version === (firmware.version ?? ""),
  );
}

function installedVersionsFor(firmware: FirmwareRecord) {
  return [
    ...new Set(
      downloadedFirmwares.value
        .filter((download) => download.id === firmware.id)
        .map((download) => download.version)
        .filter(Boolean),
    ),
  ].sort((a, b) =>
    b.localeCompare(a, undefined, { numeric: true, sensitivity: "base" }),
  );
}

function hasAvailableUpdate(firmware: FirmwareRecord) {
  return isNewerFirmwareVersion(firmware, downloadedFirmwares.value);
}

async function downloadFirmware(
  firmware: FirmwareRecord,
  installAfterDownload = false,
): Promise<boolean> {
  if (!firmware.pkg || downloadingId.value) return false;
  downloadingId.value = firmware.id;
  toast.value = null;
  try {
    const packageUrl = new URL(firmware.pkg, repositoryUrl.value);
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
      firmware.id,
      firmware.card,
      repositoryUrl.value,
    )[0];

    let path: string;
    if (isTauri()) {
      path = await invoke<string>("download_firmware", {
        url: packageUrl.toString(),
        id: firmware.id,
        version: firmware.version || "unknown",
        thumbnailUrl,
        firmwareInfo: { ...firmware, fileName: "firmware.fwsc", downloadedAt },
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
      path = "Browser downloads";
    }

    const record: DownloadedFirmware = {
      id: firmware.id,
      name: firmware.name,
      version: firmware.version ?? "",
      kind: kindLabel(firmware),
      fileName: isTauri() ? "firmware.fwsc" : fileName,
      path,
      downloadedAt,
      sha256: firmware.sha256,
    };
    const entries = [
      record,
      ...downloadedFirmwares.value.filter(
        (item) => item.id !== record.id || item.version !== record.version,
      ),
    ];
    if (isTauri()) {
      await store.set("downloadedFirmwares", entries);
      await store.save();
    } else {
      localStorage.setItem(
        "fm-mate.downloadedFirmwares",
        JSON.stringify(entries),
      );
    }
    downloadedFirmwares.value = entries;
    toast.value = {
      message: `${firmware.name} downloaded.`,
      variant: "success",
    };
    if (installAfterDownload) await beginFirmwareInstall(firmware);
    return true;
  } catch (error) {
    toast.value = {
      message: error instanceof Error ? error.message : String(error),
      variant: "error",
    };
    return false;
  } finally {
    downloadingId.value = "";
  }
}

async function beginFirmwareInstall(firmware: FirmwareRecord) {
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
    // Use the enabled default if saved settings are unavailable.
  }
  emit("openFlasher", {
    type: "switch-firmware",
    firmwareId: firmware.id,
    firmwareName: firmware.name,
    version: firmware.version ?? "unknown",
    automaticallyBackup,
  });
}

async function tryFirmware(firmware: FirmwareRecord) {
  if (!isTauri()) {
    toast.value = {
      message: "Trying firmware in FM1-Emu is available in the desktop app.",
      variant: "info",
    };
    return;
  }
  if (tryingId.value || downloadingId.value) return;
  tryingId.value = firmware.id;
  toast.value = null;
  try {
    const previouslyDownloaded = hasDownloaded(firmware);
    if (!previouslyDownloaded && !(await downloadFirmware(firmware))) return;
    toast.value = { message: "Starting emulator...", variant: "info" };
    const session = await invoke<EmulatorSavePrompt>(
      "open_firmware_in_emulator",
      {
        id: firmware.id,
        version: firmware.version ?? "unknown",
      },
    );
    const saved = await finishEmulatorSession(session);
    if (saved) {
      toast.value = {
        message: `Emulator session saved as ${saved.fileName}.${saved.warning ? ` ${saved.warning}` : ""}`,
        variant: "success",
      };
    } else {
      toast.value = {
        message: "Emulator session not saved.",
        variant: "info",
      };
    }
    if (!previouslyDownloaded) {
      firmwareKeepPrompt.value = firmware;
    }
  } catch (error) {
    toast.value = {
      message: error instanceof Error ? error.message : String(error),
      variant: "error",
    };
  } finally {
    tryingId.value = "";
  }
}

function keepTriedFirmware() {
  if (firmwareKeepPrompt.value) {
    toast.value = {
      message:
        `Kept ${firmwareKeepPrompt.value.name} ${firmwareKeepPrompt.value.version ?? ""}.`.trim(),
      variant: "success",
    };
  }
  firmwareKeepPrompt.value = null;
}

async function discardTriedFirmware() {
  const firmware = firmwareKeepPrompt.value;
  if (!firmware) return;
  firmwareKeepPrompt.value = null;
  try {
    await invoke("delete_firmware_version", {
      id: firmware.id,
      version: firmware.version ?? "unknown",
    });
    downloadedFirmwares.value = downloadedFirmwares.value.filter(
      (item) =>
        item.id !== firmware.id || item.version !== (firmware.version ?? ""),
    );
    if (isTauri()) {
      await store.set("downloadedFirmwares", downloadedFirmwares.value);
      await store.save();
    }
    toast.value = {
      message: `Removed ${firmware.name} ${firmware.version ?? ""}.`.trim(),
      variant: "success",
    };
  } catch (error) {
    toast.value = {
      message:
        error instanceof Error
          ? error.message
          : "Could not remove the test firmware.",
      variant: "error",
    };
  }
}

async function openResultInfo(firmware: FirmwareRecord) {
  await loadDownloadedFirmwares();
  const installed = downloadedFirmwares.value.some(
    (item) => item.id === firmware.id,
  );
  if (installed) {
    selectedResult.value = null;
    emit("open-installed-firmware", firmware.id);
  } else {
    selectedResult.value = firmware;
  }
}

function installResultVersion(firmware: FirmwareRecord, version: string) {
  void beginFirmwareInstall({ ...firmware, version });
}

function kindLabel(firmware: FirmwareRecord) {
  return typeof firmware.kind === "string" && firmware.kind.trim()
    ? firmware.kind
    : "Other";
}

function toggleKind(kind: string) {
  const isOnlySelectedKind =
    selectedKinds.value.length === 1 && selectedKinds.value[0] === kind;
  selectedKinds.value = isOnlySelectedKind ? availableKinds.value : [kind];
}

const availableKinds = computed(() => {
  const kinds = [...new Set(firmwares.value.map(kindLabel))];
  const counts = new Map<string, number>();
  for (const firmware of firmwares.value) {
    const kind = kindLabel(firmware);
    counts.set(kind, (counts.get(kind) ?? 0) + 1);
  }
  return kinds.sort((a, b) => (counts.get(b) ?? 0) - (counts.get(a) ?? 0));
});
const kindCounts = computed(() => {
  const counts = new Map<string, number>();
  for (const firmware of firmwares.value) {
    const kind = kindLabel(firmware);
    counts.set(kind, (counts.get(kind) ?? 0) + 1);
  }
  return counts;
});

const visibleFirmwares = computed(() => {
  const query = search.value.trim().toLocaleLowerCase();
  const filtered = firmwares.value.filter((firmware) => {
    const matchesDevice =
      deviceFilter.value === "all" ||
      (deviceFilter.value === "fm1" &&
        /FM-1/i.test(`${firmware.product ?? ""} ${firmware.ident ?? ""}`)) ||
      (deviceFilter.value === "smk37" &&
        /SMK-?37/i.test(`${firmware.product ?? ""} ${firmware.ident ?? ""}`));
    const matchesKind = selectedKinds.value.includes(kindLabel(firmware));
    const matchesSearch =
      !query ||
      [
        firmware.name,
        firmware.id,
        kindLabel(firmware),
        firmware.author,
        firmware.repo,
        firmware.what,
        firmware.product,
        firmware.version,
      ].some(
        (value) =>
          typeof value === "string" &&
          value.toLocaleLowerCase().includes(query),
      );
    return matchesDevice && matchesKind && matchesSearch;
  });

  return filtered.sort((a, b) => {
    if (sortBy.value === "stars" || sortBy.value === "installs") {
      const aValue = a[sortBy.value];
      const bValue = b[sortBy.value];
      const aValid = typeof aValue === "number" && Number.isFinite(aValue);
      const bValid = typeof bValue === "number" && Number.isFinite(bValue);
      if (aValid && bValid && aValue !== bValue)
        return sortDescending.value ? bValue - aValue : aValue - bValue;
      if (aValid !== bValid) return aValid ? -1 : 1;
      return a.name.localeCompare(b.name);
    }

    if (sortBy.value === "added" || sortBy.value === "date") {
      const aDate = Date.parse(String(a[sortBy.value] ?? ""));
      const bDate = Date.parse(String(b[sortBy.value] ?? ""));
      const aValid = Number.isFinite(aDate);
      const bValid = Number.isFinite(bDate);
      if (aValid && bValid && aDate !== bDate)
        return sortDescending.value ? bDate - aDate : aDate - bDate;
      if (aValid !== bValid) return aValid ? -1 : 1;
      return a.name.localeCompare(b.name);
    }

    const aValue = sortBy.value === "kind" ? kindLabel(a) : a.name;
    const bValue = sortBy.value === "kind" ? kindLabel(b) : b.name;
    const comparison =
      aValue.localeCompare(bValue, undefined, { sensitivity: "base" }) ||
      a.name.localeCompare(b.name);
    return sortDescending.value ? -comparison : comparison;
  });
});
</script>

<template>
  <section class="view-panel">
    <div class="view-heading">
      <div>
        <p class="eyebrow">FIRMWARE LIBRARY</p>
        <h1>Browse Firmwares</h1>
      </div>
      <div class="catalog-attribution-inline">
        <template v-if="hasRepositoryLicense">
          <span v-if="repositoryLicenseUrl"
            ><a :href="repositoryLicenseUrl" target="_blank" rel="noreferrer">{{
              repositoryLicenseDisplay
            }}</a></span
          >
          <span v-else>{{ repositoryLicenseDisplay }}</span>
          <span v-if="catalogSource"> • </span>
        </template>
        <span v-if="catalogSource"
          >Data provided by
          <a href="TheFirmwareHub.com" target="_blank" rel="noreferrer"
            >TheFirmwareHub.com</a
          ></span
        >
      </div>
    </div>

    <div class="toolbar">
      <label class="search-field"
        ><span aria-hidden="true">⌕</span
        ><input
          v-model="search"
          type="search"
          placeholder="Search name, author, device"
          aria-label="Search firmware"
      /></label>
      <select v-model="deviceFilter" aria-label="Filter by device">
        <option value="all">All devices</option>
        <option value="fm1">FM-1</option>
        <option value="smk37">SMK-37</option>
      </select>
      <ArchiveSelect
        v-if="archiveAvailable"
        :selected="selectedArchiveCommit"
        :disabled="savingArchive"
        @change="changeArchiveCommit"
      />
      <div class="result-view-toggle" role="group" aria-label="Results layout">
        <button
          type="button"
          aria-label="Grid view"
          title="Grid view"
          :aria-pressed="resultView === 'grid'"
          :class="{ 'is-active': resultView === 'grid' }"
          @click="resultView = 'grid'"
        >
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <rect x="3" y="3" width="5" height="5" rx=".5" />
            <rect x="12" y="3" width="5" height="5" rx=".5" />
            <rect x="3" y="12" width="5" height="5" rx=".5" />
            <rect x="12" y="12" width="5" height="5" rx=".5" />
          </svg>
        </button>
        <button
          type="button"
          aria-label="List view"
          title="List view"
          :aria-pressed="resultView === 'detailed'"
          :class="{ 'is-active': resultView === 'detailed' }"
          @click="resultView = 'detailed'"
        >
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <path d="M7 5h10M7 10h10M7 15h10" />
            <circle cx="3" cy="5" r=".75" />
            <circle cx="3" cy="10" r=".75" />
            <circle cx="3" cy="15" r=".75" />
          </svg>
        </button>
      </div>
    </div>
    <div v-if="archiveAvailable && archiveState.error" class="settings-feedback is-error" role="alert">
      Could not load archive dates: {{ archiveState.error }}
      <button type="button" class="outline-button" @click="loadArchiveCommits(configuredRepositoryUrl, true)">
        RETRY
      </button>
    </div>

    <div class="browse-options">
      <fieldset class="kind-filters" :disabled="!availableKinds.length">
        <legend>FILTER BY KIND</legend>
        <div>
          <div v-for="kind in availableKinds" :key="kind" class="kind-option">
            <input
              v-model="selectedKinds"
              type="checkbox"
              :value="kind"
              :aria-label="`Toggle ${kind}`"
            /><button
              type="button"
              class="kind-name"
              :title="
                selectedKinds.length === 1 && selectedKinds[0] === kind
                  ? 'Show all kinds'
                  : `Show only ${kind}`
              "
              @click="toggleKind(kind)"
            >
              {{ kind }}
              <span class="kind-count">({{ kindCounts.get(kind) ?? 0 }})</span>
            </button>
          </div>
          <span v-if="!availableKinds.length" class="no-kinds"
            >Kinds appear when a catalog is loaded.</span
          >
        </div>
      </fieldset>
      <div class="sort-controls">
        <label class="sort-control"
          >SORT BY
          <select v-model="sortBy" aria-label="Sort firmware by">
            <option value="name">Name</option>
            <option value="kind">Kind</option>
            <option value="added">Added</option>
            <option value="date">Updated</option>
            <option value="stars">Stars</option>
            <option value="installs">Installs</option>
          </select>
        </label>
        <button
          class="sort-direction"
          type="button"
          :aria-label="sortDescending ? 'Sort ascending' : 'Sort descending'"
          :title="sortDescending ? 'Sort ascending' : 'Sort descending'"
          :aria-pressed="sortDescending"
          @click="sortDescending = !sortDescending"
        >
          <span>{{ sortDescending ? "↓" : "↑" }}</span>
        </button>
      </div>
    </div>

    <div class="catalog-meta">
      <span
        v-if="repositoryUrl"
        class="catalog-source"
        :title="repositoryUrl"
        >{{ repositoryUrl }}</span
      >
      <span v-if="updated">UPDATED {{ updated }}</span>
      <span v-if="!loading && !errorMessage && repositoryUrl"
        >{{ visibleFirmwares.length }} FIRMWARE{{
          visibleFirmwares.length === 1 ? "" : "S"
        }}</span
      >
    </div>
    <Toast
      v-if="toast"
      :message="toast.message"
      :variant="toast.variant"
      :reset-key="toast"
      @dismiss="toast = null"
    />

    <div v-if="loading && firmwares.length === 0" class="catalog-state">
      <span class="empty-mark">…</span>
      <h2>Loading firmware catalog</h2>
      <p>Fetching the repository manifest.</p>
    </div>
    <div v-else-if="errorMessage" class="catalog-state catalog-state--error">
      <span class="empty-mark">!</span>
      <h2>Could not load catalog</h2>
      <p>{{ errorMessage }}</p>
      <button class="outline-button" type="button" @click="loadRepository">
        TRY AGAIN
      </button>
    </div>
    <div v-else-if="!repositoryUrl" class="catalog-state">
      <span class="empty-mark">⌁</span>
      <h2>Set a firmware repository</h2>
      <p>Add the manifest URL in Settings to browse releases.</p>
      <button
        class="outline-button"
        type="button"
        @click="emit('open-settings')"
      >
        OPEN SETTINGS
      </button>
    </div>
    <div v-else-if="visibleFirmwares.length === 0" class="catalog-state">
      <span class="empty-mark">⌕</span>
      <h2>
        {{ firmwares.length ? "No matching firmware" : "No firmware found" }}
      </h2>
      <p>
        {{
          firmwares.length
            ? "Try another search or device filter."
            : "The repository has no valid firmware entries."
        }}
      </p>
    </div>
    <div
      v-else-if="resultView === 'detailed'"
      class="firmware-list"
      aria-label="Firmware releases"
    >
      <DetailedResultCard
        v-for="firmware in visibleFirmwares"
        :key="firmware.id"
        :firmware="firmware"
        :repository-url="repositoryUrl"
        :update-available="hasAvailableUpdate(firmware)"
        :downloading="downloadingId === firmware.id"
        :downloaded="installedVersionsFor(firmware).length > 0"
        :installed-versions="installedVersionsFor(firmware)"
        :trying="tryingId === firmware.id"
        @download="downloadFirmware(firmware)"
        @try-firmware="tryFirmware(firmware)"
        @info="openResultInfo(firmware)"
        @install-version="installResultVersion(firmware, $event)"
      />
    </div>
    <div v-else class="firmware-grid" aria-label="Firmware releases">
      <FirmwareResultCard
        v-for="firmware in visibleFirmwares"
        :key="firmware.id"
        :firmware="firmware"
        :repository-url="repositoryUrl"
        :update-available="hasAvailableUpdate(firmware)"
        :busy="downloadingId === firmware.id"
        :can-install="
          !downloadingId && (hasDownloaded(firmware) || !!firmware.pkg)
        "
        :downloaded="hasDownloaded(firmware)"
        :installed-versions="installedVersionsFor(firmware)"
        @info="openResultInfo(firmware)"
        @download="downloadFirmware(firmware)"
        @install-version="installResultVersion(firmware, $event)"
      />
    </div>
    <DetailedResultCard
      v-if="selectedResult"
      :firmware="selectedResult"
      :repository-url="repositoryUrl"
      overlay
      :downloading="downloadingId === selectedResult.id"
      :downloaded="installedVersionsFor(selectedResult).length > 0"
      :installed-versions="installedVersionsFor(selectedResult)"
      :trying="tryingId === selectedResult.id"
      @download="downloadFirmware(selectedResult)"
      @try-firmware="tryFirmware(selectedResult)"
      @info="openResultInfo(selectedResult)"
      @install-version="installResultVersion(selectedResult, $event)"
      @close="selectedResult = null"
    />
    <ConfirmationDialog
      v-if="firmwareKeepPrompt"
      title="KEEP TEST FIRMWARE?"
      :message="`FM1-Emu has closed. Do you wish to keep ${firmwareKeepPrompt.name} ${firmwareKeepPrompt.version ?? ''}? Deleting it also removes its saves.`"
      confirm-label="KEEP FIRMWARE"
      cancel-label="DELETE TEST FIRMWARE"
      cancel-variant="danger"
      @confirm="keepTriedFirmware"
      @secondary="discardTriedFirmware"
      @cancel="keepTriedFirmware"
    />
    <p class="catalog-attribution">
      <template v-if="hasRepositoryLicense">
        <span v-if="repositoryLicenseUrl"
          ><a :href="repositoryLicenseUrl" target="_blank" rel="noreferrer">{{
            repositoryLicenseDisplay
          }}</a></span
        >
        <span v-else>{{ repositoryLicenseDisplay }}</span>
        <span v-if="catalogSource"> • </span>
      </template>
      <span v-if="catalogSource"
        >Data provided by
        <a :href="catalogSource.href" target="_blank" rel="noreferrer">{{
          catalogSource.label
        }}</a></span
      >
    </p>
  </section>
</template>

<style>
.view-heading {
  position: relative;
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
}
.catalog-attribution-inline {
  position: absolute;
  top: 0px;
  right: 0px;
  font-size: 11px;
  color: #aaa09a;
  white-space: nowrap;
}
.catalog-attribution-inline a {
  color: #bda5df;
  text-decoration: none;
}
.catalog-attribution-inline a:hover {
  color: #e1cff8;
  text-decoration: underline;
}
.toolbar {
  position: sticky;
  top: 120px;
  z-index: 10;
  background: rgb(28 25 24 / 95%);
  backdrop-filter: blur(4px);
  padding: 0;
  margin: 0;
}
.catalog-meta {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 8px 18px;
  min-height: 24px;
  margin: -7px 0 8px;
  color: #8f8580;
  font:
    11px "Share Tech Mono",
    monospace;
  letter-spacing: 0.04em;
}
.browse-options {
  position: sticky;
  top: 164px;
  z-index: 9;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 20px;
  margin: 0 0 14px;
  padding: 10px 12px;
  background: rgb(40 37 36 / 95%);
  border: 1px solid #48413f;
  border-top: 0;
  backdrop-filter: blur(4px);
}
@media (max-width: 1000px) {
  .toolbar {
    top: 164px;
  }
  .browse-options {
    top: 208px;
  }
}
@media (max-width: 450px) {
  .toolbar {
    top: 200px;
  }
  .browse-options {
    top: 248px;
  }
}

.kind-filters {
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}
.kind-filters > div {
  display: grid;
  min-width: 0;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  align-items: center;
  gap: 8px 0px;
}
.kind-filters legend {
  margin: 0 0 7px;
  color: #9b8aa8;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.13em;
}
.kind-filters .kind-option {
  width: 100%;
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.kind-filters .kind-name {
  padding: 0;
  color: #d2c9bd;
  background: none;
  border: 0;
  font: inherit;
  font-size: 13px;
  text-transform: capitalize;
  white-space: nowrap;
  cursor: pointer;
}
.kind-filters .kind-name:hover {
  color: #fff;
}
.kind-filters .kind-count {
  color: #9b8aa8;
}
.kind-filters input {
  width: 14px;
  height: 14px;
  margin: 0;
  accent-color: #59d3b7;
}
.kind-filters:disabled {
  opacity: 0.55;
}
.no-kinds {
  color: #8f8580;
  font:
    11px "Share Tech Mono",
    monospace;
}
.sort-controls {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 7px;
}
.sort-control {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 9px;
  color: #b49bdc;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.13em;
}
.sort-control select {
  min-width: 120px;
  height: 32px;
  padding: 0 9px;
  color: #e0d9cf;
  background: #211f1e;
  border: 1px solid #554b58;
  font:
    13px "Barlow Condensed",
    sans-serif;
}
.sort-direction {
  width: 32px;
  height: 32px;
  padding: 0;
  color: #d9c8ed;
  background: #292625;
  border: 1px solid #665772;
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
}
.sort-direction span {
  position: relative;
  top: -3px;
}
.sort-direction:hover {
  color: #fff;
  background: #332e35;
  border-color: #9b83b7;
}
.sort-direction:focus-visible {
  outline: 2px solid #65e1c1;
  outline-offset: 2px;
}
.catalog-source {
  max-width: min(72%, 700px);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.catalog-attribution {
  margin: 18px 0 0;
  color: #aaa09a;
  font:
    12px "Share Tech Mono",
    monospace;
}
.catalog-attribution a {
  color: #bda5df;
  text-decoration: none;
}
.catalog-attribution a:hover {
  color: #e1cff8;
  text-decoration: underline;
}
.catalog-state {
  min-height: min(58vh, 540px);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 25px;
  text-align: center;
  background: rgb(40 37 36 / 70%);
  border: 1px solid #48413f;
}
.catalog-state h2 {
  margin: 0 0 5px;
  color: #e3dcd2;
  font-size: 21px;
  font-weight: 600;
}
.catalog-state p {
  max-width: 62ch;
  margin: 0 0 18px;
  color: #a49b97;
  font:
    13px "Share Tech Mono",
    monospace;
  overflow-wrap: anywhere;
}
.catalog-state--error .empty-mark {
  color: #ed9a7b;
  border-color: #805b4f;
}
.firmware-list {
  display: grid;
  gap: 10px;
}
.firmware-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  align-items: stretch;
  gap: 14px;
}
.result-view-toggle {
  display: inline-flex;
  flex: 0 0 auto;
  padding: 3px;
  background: #211f1e;
  border: 1px solid #4a4442;
}
.result-view-toggle button {
  display: grid;
  width: 36px;
  min-height: 32px;
  padding: 0;
  place-items: center;
  color: #aaa09a;
  background: transparent;
  border: 0;
  cursor: pointer;
}
.result-view-toggle svg {
  width: 18px;
  height: 18px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.result-view-toggle button.is-active {
  color: #f0eae2;
  background: linear-gradient(135deg, #21413e, #1b3331);
  box-shadow: inset 0 0 0 1px #3c766a;
}
.result-view-toggle button:focus-visible {
  outline: 2px solid #65e1c1;
  outline-offset: 1px;
}
@media (max-width: 600px) {
  .catalog-source {
    max-width: 100%;
    flex-basis: 100%;
  }
  .browse-options {
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }
  .sort-controls {
    justify-content: flex-end;
  }
  .sort-control {
    justify-content: space-between;
  }
}
</style>
