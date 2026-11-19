<script setup lang="ts">
import {
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  ref,
  watch,
} from "vue";

const props = withDefaults(
  defineProps<{
    message: string;
    variant?: "success" | "error" | "info";
    resetKey?: object;
  }>(),
  { variant: "info" },
);

const emit = defineEmits<{ dismiss: [] }>();
const fading = ref(false);
const active = ref(true);
let timer: number | undefined;

function startTimer() {
  window.clearTimeout(timer);
  fading.value = false;
  timer = window.setTimeout(() => {
    fading.value = true;
    timer = window.setTimeout(() => emit("dismiss"), 250);
  }, 20_000);
}

function dismiss() {
  window.clearTimeout(timer);
  emit("dismiss");
}

onMounted(startTimer);
onActivated(() => (active.value = true));
onDeactivated(() => (active.value = false));
watch(() => [props.message, props.variant, props.resetKey], startTimer);
onBeforeUnmount(() => window.clearTimeout(timer));
</script>

<template>
  <Teleport to="#toast-viewport">
    <div
      v-if="active"
      class="toast"
      :class="[`toast--${variant}`, { 'toast--fading': fading }]"
    >
      <p :role="variant === 'error' ? 'alert' : 'status'" aria-atomic="true">
        {{ message }}<slot name="action" />
      </p>
      <button type="button" aria-label="Dismiss notification" @click="dismiss">
        ×
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.toast {
  display: flex;
  flex: 0 0 auto;
  align-items: flex-start;
  gap: 16px;
  width: 100%;
  padding: 14px 16px;
  color: #202020;
  background: #fff;
  border: 1px solid #c9c9c9;
  box-shadow: 0 8px 24px rgb(0 0 0 / 35%);
  transition: opacity 250ms ease;
}
.toast--fading {
  opacity: 0;
}
@media (prefers-reduced-motion: reduce) {
  .toast {
    transition: none;
  }
}
.toast--success {
  color: #102b1c;
  background: #8de0a8;
  border-color: #47a968;
}
.toast--error {
  color: #3a1111;
  background: #f3a5a0;
  border-color: #c65a53;
}
.toast p {
  flex: 1;
  min-width: 0;
  margin: 0;
  font: 13px/1.5 "Share Tech Mono", monospace;
  overflow-wrap: anywhere;
}
.toast button {
  flex: 0 0 auto;
  padding: 0 3px;
  color: inherit;
  background: transparent;
  border: 0;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}
.toast button:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 3px;
}
</style>
