<script setup lang="ts">
import FirmwareThumbnail from "./FirmwareThumbnail.vue";
import InstallDropdown from "./InstallDropdown.vue";
import type { FirmwareRecord } from "../services/firmwareCatalog";

defineProps<{
  firmware: FirmwareRecord;
  repositoryUrl: string;
  updateAvailable?: boolean;
  busy?: boolean;
  canInstall: boolean;
  downloaded?: boolean;
  installedVersions?: string[];
}>();
const emit = defineEmits<{
  info: [];
  install: [];
  download: [];
  installVersion: [version: string];
}>();

function formatStars(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}
</script>

<template>
  <article class="firmware-result-card">
    <button
      class="firmware-result-card__media"
      type="button"
      :aria-label="
        updateAvailable
          ? 'Open firmware information - New version!'
          : 'Open firmware information'
      "
      @click="emit('info')"
    >
      <FirmwareThumbnail
        class="firmware-result-card__image"
        :id="firmware.id"
        :card="firmware.card"
        :version="firmware.version"
        :repository-url="repositoryUrl"
        :alt="`${firmware.name} thumbnail`"
        :update-available="updateAvailable"
      />
      <div class="firmware-result-card__stats">
        <span
          v-if="formatStars(firmware.stars) !== null"
          class="firmware-result-card__stars"
          >★ {{ formatStars(firmware.stars) }}</span
        >
        <span v-if="typeof firmware.installs === 'number'"
          >{{ firmware.installs }} installs</span
        >
      </div>
    </button>
    <div class="firmware-result-card__body">
      <h2>{{ firmware.name }}</h2>
      <p class="firmware-result-card__author">
        {{ firmware.author || firmware.repo || firmware.id }}
      </p>
      <div class="firmware-result-card__description">
        {{ firmware.what || "-" }}
      </div>
      <div class="firmware-result-card__tags">
        <span>{{ firmware.kind || "Other" }}</span>
        <span v-if="firmware.lic">{{ firmware.lic }}</span>
      </div>
    </div>
    <div class="firmware-result-card__actions">
      <button class="outline-button" type="button" @click="emit('info')">
        INFO
      </button>
      <div v-if="downloaded" class="firmware-result-card__installed-actions">
        <InstallDropdown
          :versions="installedVersions ?? []"
          @select="emit('installVersion', $event)"
        />
      </div>
      <button
        v-else
        class="action-button action-button--primary"
        type="button"
        :disabled="busy || !canInstall"
        @click="emit('download')"
      >
        {{ busy ? "PREPARING…" : "DOWNLOAD" }}
      </button>
    </div>
  </article>
</template>

<style>
.firmware-result-card {
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: rgb(40 37 36 / 88%);
  border: 1px solid #48413f;
}
.firmware-result-card__media {
  position: relative;
  display: block;
  width: 100%;
  aspect-ratio: 1.4;
  overflow: hidden;
  padding: 0;
  background: #211f1e;
  border: 0;
  text-align: left;
  cursor: pointer;
}
.firmware-result-card__media > .firmware-result-card__image {
  width: 100%;
  height: 100%;
  aspect-ratio: auto;
  flex-basis: auto;
  border: 0;
}
.firmware-result-card__image img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.firmware-result-card__stats {
  position: absolute;
  left: 10px;
  bottom: 10px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.firmware-result-card__stats span {
  padding: 4px 7px;
  color: #d6e1e3;
  background: rgb(24 28 30 / 88%);
  border: 1px solid rgb(180 195 195 / 45%);
  font:
    11px "Share Tech Mono",
    monospace;
}
.firmware-result-card__stats .firmware-result-card__stars {
  color: #f1c66e;
  border-color: #74603b;
}
.firmware-result-card__body {
  display: grid;
  align-content: start;
  gap: 8px;
  min-width: 0;
  padding: 13px 14px 8px;
}
.firmware-result-card__body h2 {
  margin: 0;
  color: #e8e1d7;
  font-size: 18px;
  line-height: 1.15;
}
.firmware-result-card__author {
  margin: 0;
  overflow: hidden;
  color: #b49bdc;
  font:
    11px "Share Tech Mono",
    monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.firmware-result-card__description {
  height: 4.5em;
  margin: 0;
  overflow: hidden;
  overflow-wrap: anywhere;
  color: #b9b0a7;
  font:
    12px/1.5 "Share Tech Mono",
    monospace;
  -webkit-mask-image: linear-gradient(to bottom, #000 66.67%, transparent);
  mask-image: linear-gradient(to bottom, #000 66.67%, transparent);
}
.firmware-result-card__tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.firmware-result-card__tags span {
  padding: 3px 7px;
  color: #c4b9cc;
  background: #302d30;
  border: 1px solid #514754;
  font:
    10px "Share Tech Mono",
    monospace;
}
.firmware-result-card__actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-top: auto;
  padding: 8px 14px 14px;
}
.firmware-result-card__actions button {
  min-height: 38px;
  justify-content: center;
  padding: 6px 8px;
  font-size: 11px;
}
.firmware-result-card__installed-actions {
  min-width: 0;
}
.firmware-result-card__installed-actions .install-dropdown {
  width: 100%;
  min-width: 0;
}
.firmware-result-card__installed-actions .install-button {
  min-height: 38px;
}
</style>
