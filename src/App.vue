<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";
import { openUrl } from "@tauri-apps/plugin-opener";
import AppHeader from "./components/AppHeader.vue";
import WelcomeDialog from "./components/WelcomeDialog.vue";
import EmulatorSaveDialog from "./components/EmulatorSaveDialog.vue";
import Toast from "./components/Toast.vue";
import BrowseFirmwares from "./views/BrowseFirmwares.vue";
import DownloadedFirmwares from "./views/DownloadedFirmwares.vue";
import BackupsSaves from "./views/BackupsSaves.vue";
import SettingsView from "./views/SettingsView.vue";
import MFlasherView from "./views/MFlasherView.vue";
import { appVersion, isNewerAppVersion } from "./services/appVersion";
import { getAppRelease } from "./services/appRelease";
import {
  findFirmwareUpdate,
  getInstalledFirmwares,
  loadFirmwareCatalog,
} from "./services/firmwareCatalog";
import type {
  DownloadedFirmware,
  FirmwareCatalog,
} from "./services/firmwareCatalog";
import { FIRMWARE_CATALOG_REFRESHED_EVENT } from "./config";
import type {
  MFlasherLcdState,
  MFlasherRequest,
  MFlasherRequestPayload,
} from "./types/flasher";

type View = "browse" | "downloaded" | "backups" | "settings" | "lcd";
interface SaveFilter {
  firmwareId: string;
  version: string;
}
const activeView = ref<View>("browse");
const welcomeVisible = ref(false);
const welcomeMode = ref<"newuser" | "changelog">("newuser");
const appStateLoaded = ref(false);
const appUpdateAvailable = ref(false);
const appUpdateOpenError = ref("");
const firmwareUpdateToast = ref<{ message: string } | null>(null);
const saveFilter = ref<SaveFilter | null>(null);
const requestedFirmwareDetailsId = ref<string | null>(null);
const requestedUpdatesFilter = ref<number | null>(null);
let nextUpdatesFilterRequestId = 0;
const flasherRequest = ref<MFlasherRequest | null>(null);
const flasherLcdState = ref<MFlasherLcdState>({
  title: "NO UNIT",
  message: "WAITING FOR DEVICE",
  connection: "USB",
  hint: "HOLD SEQ REC (SMK-37) OR REC (FM-1) WHILE SWITCHING ON",
  progress: null,
  messageClass: "",
});
let nextFlasherRequestId = 0;
const libraryStore = new LazyStore("settings.json", { autoSave: true });
const validViews: View[] = [
  "browse",
  "downloaded",
  "backups",
  "settings",
  "lcd",
];
const activeComponent = computed(
  () =>
    ({
      browse: BrowseFirmwares,
      downloaded: DownloadedFirmwares,
      backups: BackupsSaves,
      settings: SettingsView,
      lcd: MFlasherView,
    })[activeView.value],
);

function openSaves(filter: SaveFilter) {
  saveFilter.value = filter;
  activeView.value = "backups";
}

function openInstalledFirmwareDetails(firmwareId: string) {
  requestedFirmwareDetailsId.value = firmwareId;
  activeView.value = "downloaded";
}

function clearRequestedFirmwareDetails(firmwareId: string) {
  if (requestedFirmwareDetailsId.value === firmwareId)
    requestedFirmwareDetailsId.value = null;
}

function openFirmwareUpdates() {
  requestedFirmwareDetailsId.value = null;
  requestedUpdatesFilter.value = ++nextUpdatesFilterRequestId;
  activeView.value = "downloaded";
  firmwareUpdateToast.value = null;
}

function clearRequestedUpdatesFilter(requestId: number) {
  if (requestedUpdatesFilter.value === requestId)
    requestedUpdatesFilter.value = null;
}

function openFlasher(request: MFlasherRequestPayload) {
  flasherRequest.value = {
    ...request,
    requestId: ++nextFlasherRequestId,
  } as MFlasherRequest;
  activeView.value = "lcd";
}

function clearFlasherRequest(requestId: number) {
  if (flasherRequest.value?.requestId === requestId)
    flasherRequest.value = null;
}

async function openAppRelease() {
  appUpdateOpenError.value = "";
  try {
    await openUrl("https://github.com/EJTH/FM-MATE/releases/latest");
  } catch (error) {
    console.error("Could not open the app download page:", error);
    appUpdateOpenError.value = "Could not open the download page.";
  }
}

let firmwareUpdateCheckId = 0;
let lastFirmwareCatalogKey = "";
async function checkFirmwareUpdates(catalog: FirmwareCatalog) {
  if (catalog.stale) return;
  const catalogKey = `${catalog.repositoryUrl}:${catalog.fetchedAt}`;
  if (catalogKey === lastFirmwareCatalogKey) return;
  lastFirmwareCatalogKey = catalogKey;
  const checkId = ++firmwareUpdateCheckId;
  try {
    const installed: DownloadedFirmware[] = isTauri()
      ? await getInstalledFirmwares()
      : JSON.parse(localStorage.getItem("fm-mate.downloadedFirmwares") ?? "[]");
    if (checkId !== firmwareUpdateCheckId) return;
    const ids = new Set(installed.map((firmware) => firmware.id));
    firmwareUpdateToast.value = [...ids].some((id) =>
      findFirmwareUpdate(id, catalog.firmwares, installed),
    )
      ? {
          message: "There are updates for your downloaded firmware,",
        }
      : null;
  } catch (error) {
    if (catalogKey === lastFirmwareCatalogKey) lastFirmwareCatalogKey = "";
    console.error("Could not check downloaded firmware for updates:", error);
  }
}

function onFirmwareCatalogRefreshed(event: Event) {
  if (event instanceof CustomEvent) {
    void checkFirmwareUpdates(event.detail as FirmwareCatalog);
  }
}

onMounted(() => {
  window.addEventListener(
    FIRMWARE_CATALOG_REFRESHED_EVENT,
    onFirmwareCatalogRefreshed,
  );
  void loadFirmwareCatalog()
    .then(checkFirmwareUpdates)
    .catch((error: unknown) =>
      console.error("Could not load the firmware catalog on launch:", error),
    );
});
onBeforeUnmount(() => {
  firmwareUpdateCheckId++;
  window.removeEventListener(
    FIRMWARE_CATALOG_REFRESHED_EVENT,
    onFirmwareCatalogRefreshed,
  );
});

onMounted(async () => {
  if (!isTauri()) return;
  try {
    const release = await getAppRelease();
    appUpdateAvailable.value = release.updateInfo?.isUpdateAvailable ?? false;
  } catch (error) {
    console.error("Could not check for app updates on launch:", error);
  }
});

onMounted(async () => {
  try {
    let hasLaunched: boolean;
    let previousVersion: string | null | undefined;
    if (isTauri()) {
      const savedView = await libraryStore.get<string>("lastActiveView");
      if (savedView && validViews.includes(savedView as View))
        activeView.value = savedView as View;
      hasLaunched =
        (await libraryStore.get<boolean>("hasCompletedWelcome")) === true;
      previousVersion = await libraryStore.get<string>("lastWelcomeVersion");
    } else {
      const savedView = localStorage.getItem("fm-mate.lastActiveView");
      if (savedView && validViews.includes(savedView as View))
        activeView.value = savedView as View;
      hasLaunched =
        localStorage.getItem("fm-mate.hasCompletedWelcome") === "true";
      previousVersion = localStorage.getItem("fm-mate.lastWelcomeVersion");
    }
    const updated =
      hasLaunched &&
      (!previousVersion || isNewerAppVersion(appVersion, previousVersion));
    welcomeMode.value = hasLaunched ? "changelog" : "newuser";
    welcomeVisible.value = import.meta.env.DEV || !hasLaunched || updated;
  } catch (error) {
    console.error("Could not load welcome state:", error);
    welcomeVisible.value = true;
  }
  appStateLoaded.value = true;

  if (!isTauri()) return;
  try {
    const installed = await invoke<unknown[]>("scan_installed_firmwares");
    await libraryStore.set("downloadedFirmwares", installed);
    await libraryStore.save();
  } catch {
    // The firmware views report their own directory scan errors when opened.
  }
});

watch(activeView, async (view) => {
  if (!appStateLoaded.value) return;
  try {
    if (isTauri()) {
      await libraryStore.set("lastActiveView", view);
      await libraryStore.save();
    } else {
      localStorage.setItem("fm-mate.lastActiveView", view);
    }
  } catch {
    // Tab persistence is best effort; navigation remains available if storage fails.
  }
});

async function dismissWelcome() {
  welcomeVisible.value = false;
  try {
    if (isTauri()) {
      await libraryStore.set("hasCompletedWelcome", true);
      await libraryStore.set("lastWelcomeVersion", appVersion);
      await libraryStore.save();
    } else {
      localStorage.setItem("fm-mate.hasCompletedWelcome", "true");
      localStorage.setItem("fm-mate.lastWelcomeVersion", appVersion);
    }
  } catch (error) {
    console.error("Could not save welcome state:", error);
  }
}
</script>

<template>
  <main class="app-shell">
    <div id="toast-viewport" class="toast-viewport"></div>
    <AppHeader
      :active-view="activeView"
      :lcd-state="flasherLcdState"
      @navigate="activeView = $event"
    />
    <WelcomeDialog
      v-if="welcomeVisible"
      :mode="welcomeMode"
      :version="appVersion"
      @dismiss="dismissWelcome"
    />
    <EmulatorSaveDialog />
    <div class="app-content">
      <KeepAlive>
        <component
          :is="activeComponent"
          :key="activeView"
          role="tabpanel"
          :initial-filter="activeView === 'backups' ? saveFilter : null"
          :open-firmware-id="
            activeView === 'downloaded' ? requestedFirmwareDetailsId : null
          "
          :show-updates-request="
            activeView === 'downloaded' ? requestedUpdatesFilter : null
          "
          :pending-action="activeView === 'lcd' ? flasherRequest : null"
          @open-settings="activeView = 'settings'"
          @open-saves="openSaves"
          @open-flasher="openFlasher"
          @open-installed-firmware="openInstalledFirmwareDetails"
          @details-opened="clearRequestedFirmwareDetails"
          @updates-filter-applied="clearRequestedUpdatesFilter"
          @action-complete="clearFlasherRequest"
          @lcd-state="flasherLcdState = $event"
        />
      </KeepAlive>
    </div>
    <Toast
      v-if="firmwareUpdateToast && appStateLoaded && !welcomeVisible"
      :message="firmwareUpdateToast.message"
      :reset-key="firmwareUpdateToast"
      variant="info"
      @dismiss="firmwareUpdateToast = null"
    >
      <template #action>
        <a
          class="app-update-link"
          href="#library"
          @click.prevent="openFirmwareUpdates"
          >check your library.</a
        >
      </template>
    </Toast>
    <Toast
      v-if="appUpdateAvailable && appStateLoaded && !welcomeVisible"
      message="An FM-MATE update is available."
      variant="info"
      @dismiss="appUpdateAvailable = false"
    >
      <template #action>
        <a
          class="app-update-link"
          href="https://github.com/EJTH/FM-MATE/releases/latest"
          target="_blank"
          rel="noopener noreferrer"
          @click.prevent="openAppRelease"
        >Click here to Download</a>
        <span v-if="appUpdateOpenError"> {{ appUpdateOpenError }}</span>
      </template>
    </Toast>
  </main>
</template>

<style>
@import "./theme.css";
html {
  scroll-behavior: smooth;
  overflow: auto;
  overflow-y: scroll;
}
#app {
  min-height: 100vh;
}
.app-shell {
  min-height: 100vh;
  padding-bottom: 48px;
}
.toast-viewport {
  position: fixed;
  z-index: 110;
  right: 20px;
  bottom: 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: min(400px, calc(100vw - 40px));
  max-height: calc(100vh - 40px);
  overflow-y: auto;
}
.app-update-link {
  margin-left: 0.4em;
  color: inherit;
}
.app-content {
  width: min(100%, 1500px);
  margin: 0 auto;
  padding-inline: clamp(18px, 3vw, 44px);
}
@media (max-width: 600px) {
  .app-content {
    padding-inline: 14px;
  }
}
</style>
