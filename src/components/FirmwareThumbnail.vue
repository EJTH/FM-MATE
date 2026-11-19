<script setup lang="ts">
import { onActivated, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import NewVersionBanner from "./NewVersionBanner.vue";
import { firmwareThumbnailUrls } from "../services/firmwareCatalog";
import { THUMBNAIL_CACHE_CLEARED_EVENT, THUMBNAIL_CACHE_NAME } from "../config";

const props = defineProps<{
  id: string;
  card?: string | null;
  alt: string;
  repositoryUrl: string;
  version?: string;
  fallbackData?: string | null;
  updateAvailable?: boolean;
}>();
const imageSource = ref("");
const failed = ref(false);
const element = ref<HTMLDivElement | null>(null);
let objectUrl = "";
let observer: IntersectionObserver | undefined;
let visible = false;
let loadGeneration = 0;
let refreshAfterCacheClear = false;

async function loadBrowserImage(
  url: string,
  version: string,
  generation: number,
) {
  const cache =
    "caches" in window ? await caches.open(THUMBNAIL_CACHE_NAME) : undefined;
  const request = new Request(url);
  const cacheUrl = new URL(url);
  cacheUrl.searchParams.append("__fm_mate_version", version);
  const cacheKey = new Request(cacheUrl);
  let response = await cache?.match(cacheKey);
  if (!response) {
    response = await fetch(request, { cache: "no-cache" });
    if (!response.ok)
      throw new Error(`Thumbnail request failed (${response.status}).`);
    if (generation === loadGeneration)
      await cache?.put(cacheKey, response.clone());
  }
  return URL.createObjectURL(await response.blob());
}

async function loadImage() {
  const generation = ++loadGeneration;
  failed.value = false;
  imageSource.value = "";
  if (objectUrl) URL.revokeObjectURL(objectUrl);
  objectUrl = "";
  if (!props.repositoryUrl) {
    imageSource.value = props.fallbackData ?? "";
    return;
  }
  let candidates: string[];
  try {
    candidates = firmwareThumbnailUrls(
      props.id,
      props.card,
      props.repositoryUrl,
    );
  } catch (error) {
    console.warn("Could not resolve firmware thumbnail URL:", error);
    if (props.fallbackData) imageSource.value = props.fallbackData;
    else failed.value = true;
    return;
  }
  let lastError: unknown;
  for (const candidate of candidates) {
    try {
      const browserImage = !isTauri();
      const source = browserImage
        ? await loadBrowserImage(candidate, props.version ?? "", generation)
        : await invoke<string>("get_firmware_thumbnail", {
            id: props.id,
            imageUrl: candidate,
            version: props.version ?? "",
          });
      if (generation !== loadGeneration) {
        if (browserImage) URL.revokeObjectURL(source);
        return;
      }
      if (browserImage) objectUrl = source;
      imageSource.value = source;
      return;
    } catch (error) {
      if (generation !== loadGeneration) return;
      lastError = error;
    }
  }
  console.warn("Could not load firmware thumbnail:", lastError);
  if (props.fallbackData) imageSource.value = props.fallbackData;
  else failed.value = true;
}

watch(
  () => [
    props.id,
    props.card,
    props.repositoryUrl,
    props.version,
    props.fallbackData,
  ],
  () => {
    if (visible) void loadImage();
  },
);

function markCacheCleared() {
  loadGeneration++;
  refreshAfterCacheClear = true;
}

onActivated(() => {
  if (refreshAfterCacheClear) {
    refreshAfterCacheClear = false;
    if (visible) void loadImage();
  }
});

onMounted(() => {
  window.addEventListener(THUMBNAIL_CACHE_CLEARED_EVENT, markCacheCleared);
  if (!("IntersectionObserver" in window) || !element.value) {
    visible = true;
    void loadImage();
    return;
  }
  observer = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        observer?.disconnect();
        visible = true;
        void loadImage();
      }
    },
    { rootMargin: "160px" },
  );
  observer.observe(element.value);
});
onBeforeUnmount(() => {
  window.removeEventListener(THUMBNAIL_CACHE_CLEARED_EVENT, markCacheCleared);
  loadGeneration++;
  observer?.disconnect();
  if (objectUrl) URL.revokeObjectURL(objectUrl);
});
</script>

<template>
  <div
    ref="element"
    class="firmware-thumbnail"
    :class="{ 'firmware-thumbnail--missing': failed }"
  >
    <img
      v-if="imageSource && !failed"
      :src="imageSource"
      :alt="alt"
      loading="lazy"
    />
    <span v-else class="firmware-thumbnail__placeholder" aria-hidden="true"
      >⌁</span
    >
    <NewVersionBanner v-if="updateAvailable" />
  </div>
</template>

<style>
.firmware-thumbnail {
  position: relative;
  display: grid;
  width: 156px;
  height: 156px;
  flex: 0 0 156px;
  place-items: center;
  overflow: hidden;
  color: #9f8cb1;
  background: #211f1e;
  border: 1px solid #514754;
}
.firmware-thumbnail img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.firmware-thumbnail__placeholder {
  font-size: 28px;
}
</style>
