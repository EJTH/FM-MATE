<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'

const props = defineProps<{ id: string; card?: string | null; alt: string }>()
const imageSource = ref('')
const failed = ref(false)
const element = ref<HTMLDivElement | null>(null)
let objectUrl = ''
let observer: IntersectionObserver | undefined

async function loadBrowserImage(url: string) {
  const cache = 'caches' in window ? await caches.open('fm-mate-thumbnails-v1') : undefined
  const request = new Request(url)
  let response = await cache?.match(request)
  if (!response) {
    response = await fetch(request)
    if (!response.ok) throw new Error(`Thumbnail request failed (${response.status}).`)
    await cache?.put(request, response.clone())
  }
  objectUrl = URL.createObjectURL(await response.blob())
  imageSource.value = objectUrl
}

async function loadImage() {
  const candidates = props.card?.trim()
    ? [
        { url: new URL(props.card, 'https://thefirmwarehub.com/').toString(), isCard: true },
        { url: `https://thefirmwarehub.com/img/w0/${encodeURIComponent(props.id)}.webp`, isCard: false },
      ]
    : [{ url: `https://thefirmwarehub.com/img/w0/${encodeURIComponent(props.id)}.webp`, isCard: false }]

  for (const candidate of candidates) {
    try {
      if (isTauri()) {
        imageSource.value = await invoke<string>('get_firmware_thumbnail', {
          id: props.id,
          imageUrl: candidate.url,
          isCard: candidate.isCard,
        })
      } else {
        await loadBrowserImage(candidate.url)
      }
      return
    } catch {
      // Keep the ID thumbnail as a fallback if the card image is unavailable.
    }
  }
  failed.value = true
}

onMounted(() => {
  if (!('IntersectionObserver' in window) || !element.value) {
    void loadImage()
    return
  }
  observer = new IntersectionObserver((entries) => {
    if (entries.some((entry) => entry.isIntersecting)) {
      observer?.disconnect()
      void loadImage()
    }
  }, { rootMargin: '160px' })
  observer.observe(element.value)
})
onBeforeUnmount(() => {
  observer?.disconnect()
  if (objectUrl) URL.revokeObjectURL(objectUrl)
})
</script>

<template>
  <div ref="element" class="firmware-thumbnail" :class="{ 'firmware-thumbnail--missing': failed }">
    <img v-if="imageSource && !failed" :src="imageSource" :alt="alt" loading="lazy" />
    <span v-else aria-hidden="true">⌁</span>
  </div>
</template>

<style>
.firmware-thumbnail { display: grid; width: 156px; height: 156px; flex: 0 0 156px; place-items: center; overflow: hidden; color: #9f8cb1; background: #211f1e; border: 1px solid #514754; }
.firmware-thumbnail img { display: block; width: 100%; height: 100%; object-fit: cover; }
.firmware-thumbnail span { font-size: 28px; }
</style>
