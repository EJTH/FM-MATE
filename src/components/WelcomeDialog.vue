<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import ToolDownloadsSection from "./ToolDownloadsSection.vue";

const props = defineProps<{ mode: "newuser" | "changelog"; version: string }>();
const emit = defineEmits<{ dismiss: [] }>();
const changelog = ref("");
const changelogError = ref("");
const loadingChangelog = ref(false);
const releaseController = new AbortController();

onMounted(async () => {
  if (props.mode !== "changelog") return;
  loadingChangelog.value = true;
  try {
    const response = await fetch(
      `https://api.github.com/repos/EJTH/FM-MATE/releases`,
      {
        headers: { Accept: "application/vnd.github+json" },
        signal: releaseController.signal,
      },
    );
    console.log(response);
    if (!response.ok)
      throw new Error(`GitHub returned HTTP ${response.status}.`);
    const releases: Array<{ body?: unknown }> = await response.json();
    const release = releases.find((item) => item.body);
    if (typeof release?.body !== "string" || !release?.body.trim())
      throw new Error("This release has no notes.");
    changelog.value = release.body
      .trim()
      .replace(/#+(.+)\n/g, (_, title) => `<h3>${title}</h3>`);
  } catch (error) {
    if (!releaseController.signal.aborted) {
      console.error("Could not load release notes:", error);
      changelogError.value =
        "Could not load release notes. Check your connection or visit the releases page.";
    }
  } finally {
    loadingChangelog.value = false;
  }
});

onBeforeUnmount(() => releaseController.abort());
</script>

<template>
  <div
    class="welcome-backdrop"
    role="presentation"
    @click.self="emit('dismiss')"
    @keydown.esc="emit('dismiss')"
    tabindex="-1"
  >
    <section
      class="welcome-window"
      role="dialog"
      aria-modal="true"
      aria-labelledby="welcome-title"
    >
      <header class="welcome-titlebar">
        <span class="welcome-brand">FM-MATE</span>
        <strong id="welcome-title">WELCOME</strong>
        <button
          type="button"
          aria-label="Close welcome dialog"
          @click="emit('dismiss')"
        >
          ×
        </button>
      </header>
      <div class="welcome-content">
        <img src="/fm-mate-logo.svg" alt="FM-MATE Logo" class="welcome-logo" />
        <div v-if="mode === 'newuser'" class="welcome-newuser">
          <p class="welcome-thanks">Thanks for downloading FM-MATE!</p>
          <p class="welcome-version">{{ version }}</p>
          <p class="welcome-intro">
            Before you get started, be sure to download the necessary tools
            below. And to prevent any bricking mishaps, go to settings and
            install m-bootloader on your FM-1! m-bootloader prevents your device
            from being bricked by ensuring that it can always go into
            "download-mode" -
          </p>
          <div class="m-bootloader-warning">
            Flash m-bootloader to your device through the Settings menu before
            you switch firmwares!
          </div>
        </div>
        <div v-else class="welcome-changelog">
          <p class="welcome-thanks">What's new in FM-MATE</p>
          <p v-if="loadingChangelog" role="status">Loading release notes...</p>
          <p v-else-if="changelogError" role="status">
            {{ changelogError }}
            <a
              href="https://github.com/EJTH/FM-MATE/releases"
              target="_blank"
              rel="noopener noreferrer"
              >View releases</a
            >
          </p>
          <div v-else class="welcome-changelog-body" v-html="changelog"></div>
        </div>
        <ToolDownloadsSection
          title="TOOLS YOU SHOULD DOWNLOAD"
          description="M-Flasher is required to install firmware and create device backups. FM1-Emu is optional and lets you try firmware locally before using it on your device."
        />
        <footer>
          <button
            class="action-button action-button--primary"
            type="button"
            @click="emit('dismiss')"
          >
            GET STARTED
          </button>
        </footer>
      </div>
    </section>
  </div>
</template>

<style>
.welcome-backdrop {
  position: fixed;
  z-index: 80;
  inset: 0;
  display: grid;
  place-items: center;
  overflow: auto;
  padding: 20px;
  background: rgb(0 0 0 / 76%);
}
.welcome-window {
  width: min(760px, 100%);
  color: #e5dfd5;
  background: #292726;
  border: 1px solid #65586a;
  box-shadow: 0 18px 60px rgb(0 0 0 / 65%);
}
.welcome-content {
  overflow: auto;
  max-height: 80vh;
  min-height: 20vh;
}
.welcome-titlebar {
  position: sticky;
  top: 0;
  min-height: 43px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 9px 6px 13px;
  background: linear-gradient(90deg, #211f1e, #302b30);
  border-bottom: 1px solid #514754;
}
.welcome-brand {
  color: #72e7c3;
  font:
    italic 800 13px Arial,
    sans-serif;
  letter-spacing: -0.04em;
}
.welcome-titlebar strong {
  min-width: 0;
  flex: 1;
  color: #c9b8df;
  font-size: 11px;
  letter-spacing: 0.13em;
}
.welcome-titlebar button {
  width: 29px;
  height: 29px;
  color: #d5c8d7;
  background: #382e35;
  border: 1px solid #665364;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}
.welcome-content {
  padding: 22px;
}
.welcome-logo {
  width: 240px;
  height: 240px;
  margin: 0 auto 20px;
  display: block;
  object-fit: contain;
  filter: drop-shadow(-10px 15px 4px rgb(0 0 0 / 55%));
}
.welcome-thanks {
  margin: 0 0 7px;
  color: #72e7c3;
  font-size: 22px;
  font-weight: 700;
}
.welcome-version {
  margin: 0 0 14px;
  color: #b49bdc;
  font:
    13px "Share Tech Mono",
    monospace;
}
.welcome-changelog {
  margin-bottom: 20px;
}
.welcome-changelog a {
  color: #bda5df;
}
.welcome-changelog-body h3 {
  display: blocK;
  color: #bda5df;
  font-weight: bold;
  font-size: 14px;
  margin: 0px 0px;
  padding: 0px 0px;
}
.welcome-changelog-body {
  margin-top: 20px;
  max-height: 200px;
  overflow: auto;
  padding: 12px;
  color: #b9b0a7;
  background: #211f1e;
  border: 1px solid #48413f;
  font:
    12px/1 "Share Tech Mono",
    monospace;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.welcome-intro {
  margin: 0 0 18px;
  color: #d5cec6;
  font:
    13px/1.6 "Share Tech Mono",
    monospace;
}
.welcome-content .settings-group {
  padding: 0;
  border: 0;
}
.welcome-content footer {
  display: flex;
  justify-content: flex-end;
  margin-top: 14px;
}
.welcome-content footer button {
  min-height: 40px;
  padding: 8px 18px;
  cursor: pointer;
}
.welcome-content .m-bootloader-warning {
  margin-bottom: 20px;
  font-size: 20px;
  color: #ff6b6b;
}
</style>
