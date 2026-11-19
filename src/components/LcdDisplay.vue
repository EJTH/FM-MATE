<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    title?: string;
    message?: string;
    connection?: string;
    hint?: string;
    messageClass?: string;
    progress?: number | null;
  }>(),
  {
    title: "NO UNIT",
    message: "WAITING FOR DOWNLOAD MODE",
    connection: "USB",
    hint: "HOLD SEQ REC (SMK-37) OR REC (FM-1) WHILE SWITCHING ON",
    progress: null,
  },
);

const titleElement = ref<HTMLElement | null>(null);
let resizeObserver: ResizeObserver | undefined;

function fitTitle() {
  const element = titleElement.value;
  if (!element || !element.clientWidth) return;
  element.style.removeProperty("font-size");
  if (element.scrollWidth <= element.clientWidth) return;

  let min = Math.min(8, parseFloat(getComputedStyle(element).fontSize));
  let max = parseFloat(getComputedStyle(element).fontSize);
  for (let i = 0; i < 12; i++) {
    const size = (min + max) / 2;
    element.style.fontSize = `${size}px`;
    if (element.scrollWidth <= element.clientWidth) min = size;
    else max = size;
  }
  element.style.fontSize = `${min}px`;
}

watch(() => props.title, fitTitle, { flush: "post" });
onMounted(() => {
  resizeObserver = new ResizeObserver(fitTitle);
  if (titleElement.value) resizeObserver.observe(titleElement.value);
  document.fonts.addEventListener("loadingdone", fitTitle);
  fitTitle();
});
onUnmounted(() => {
  resizeObserver?.disconnect();
  document.fonts.removeEventListener("loadingdone", fitTitle);
});
</script>

<template>
  <section class="lcd-bezel" aria-label="Device LCD display">
    <div class="lcd-screen">
      <slot name="graphic">
        <div class="lcd-device-icon" aria-hidden="true">
          <span v-if="progress === null" />
          <span v-else class="lcd-progress-value">{{ progress }}%</span>
        </div>
      </slot>
      <div class="lcd-content">
        <slot>
          <h2 ref="titleElement" class="lcd-title">{{ title }}</h2>
          <p class="lcd-message" :class="messageClass" aria-live="polite">
            {{ message }}
          </p>
          <p class="lcd-connection">{{ connection }}</p>
        </slot>
      </div>
      <p class="lcd-hint">
        <slot name="hint">{{ hint }}</slot>
      </p>
    </div>
  </section>
</template>

<style scoped>
.lcd-title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
