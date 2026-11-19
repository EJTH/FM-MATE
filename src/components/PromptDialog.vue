<script setup lang="ts">
import { onMounted, ref, useId } from "vue";

withDefaults(
  defineProps<{
    title: string;
    label: string;
    description?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    errorMessage?: string;
    disabled?: boolean;
  }>(),
  {
    confirmLabel: "CONFIRM",
    cancelLabel: "CANCEL",
    errorMessage: "",
    disabled: false,
  },
);
const value = defineModel<string>({ required: true });
const emit = defineEmits<{ submit: []; cancel: [] }>();
const inputId = useId();
const input = ref<HTMLInputElement | null>(null);

onMounted(() => {
  input.value?.focus();
  input.value?.select();
});
</script>

<template>
  <div
    class="prompt-backdrop"
    role="presentation"
    tabindex="-1"
    @click.self="emit('cancel')"
    @keydown.esc="emit('cancel')"
  >
    <section
      class="prompt-window"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="`${inputId}-title`"
    >
      <header class="prompt-titlebar">
        <span class="prompt-brand">FM-MATE</span>
        <strong :id="`${inputId}-title`">{{ title }}</strong>
        <button
          type="button"
          aria-label="Close dialog"
          :disabled="disabled"
          @click="emit('cancel')"
        >
          ×
        </button>
      </header>
      <form class="prompt-content" @submit.prevent="emit('submit')">
        <p v-if="description" class="prompt-description">{{ description }}</p>
        <label :for="inputId">{{ label }}</label>
        <input
          :id="inputId"
          ref="input"
          v-model="value"
          type="text"
          autocomplete="off"
          :disabled="disabled"
        />
        <p v-if="errorMessage" class="prompt-error" role="alert">
          {{ errorMessage }}
        </p>
        <footer>
          <button
            type="button"
            class="outline-button"
            :disabled="disabled"
            @click="emit('cancel')"
          >
            {{ cancelLabel }}
          </button>
          <button
            type="submit"
            class="action-button action-button--primary"
            :disabled="disabled"
          >
            {{ confirmLabel }}
          </button>
        </footer>
      </form>
    </section>
  </div>
</template>

<style>
.prompt-backdrop {
  position: fixed;
  z-index: 100;
  inset: 0;
  display: grid;
  place-items: center;
  overflow: auto;
  padding: 20px;
  background: rgb(0 0 0 / 72%);
}
.prompt-window {
  width: min(480px, 100%);
  color: #e5dfd5;
  background: #292726;
  border: 1px solid #65586a;
  box-shadow: 0 18px 60px rgb(0 0 0 / 65%);
}
.prompt-titlebar {
  min-height: 43px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 9px 6px 13px;
  background: linear-gradient(90deg, #211f1e, #302b30);
  border-bottom: 1px solid #514754;
}
.prompt-brand {
  color: #72e7c3;
  font:
    italic 800 13px Arial,
    sans-serif;
  letter-spacing: -0.04em;
}
.prompt-titlebar strong {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: #c9b8df;
  font-size: 11px;
  letter-spacing: 0.13em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.prompt-titlebar button {
  width: 29px;
  height: 29px;
  color: #d5c8d7;
  background: #382e35;
  border: 1px solid #665364;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}
.prompt-titlebar button:disabled {
  opacity: 0.55;
  cursor: wait;
}
.prompt-content {
  display: grid;
  gap: 12px;
  padding: 20px;
}
.prompt-content label,
.prompt-description,
.prompt-error {
  color: #d5cec6;
  font:
    13px/1.6 "Share Tech Mono",
    monospace;
}
.prompt-description,
.prompt-error {
  margin: 0;
}
.prompt-content input {
  width: 100%;
  padding: 8px 10px;
  color: #e3dcd2;
  background: #211f1e;
  border: 1px solid #665772;
  font:
    13px "Share Tech Mono",
    monospace;
}
.prompt-error {
  color: #efaaa3;
  overflow-wrap: anywhere;
}
.prompt-content footer {
  display: flex;
  justify-content: flex-end;
  gap: 9px;
  margin-top: 10px;
}
.prompt-content button {
  min-height: 38px;
  padding: 7px 12px;
  font-size: 11px;
  cursor: pointer;
}
.prompt-content button:disabled {
  opacity: 0.55;
  cursor: wait;
}
</style>
