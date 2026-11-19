<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    confirmVariant?: "primary" | "danger";
    cancelVariant?: "outline" | "danger";
  }>(),
  {
    confirmLabel: "CONFIRM",
    cancelLabel: "CANCEL",
    confirmVariant: "primary",
    cancelVariant: "outline",
  },
);
const emit = defineEmits<{ confirm: []; secondary: []; cancel: [] }>();
</script>

<template>
  <div
    class="confirmation-backdrop"
    role="presentation"
    tabindex="-1"
    @click.self="emit('cancel')"
    @keydown.esc="emit('cancel')"
  >
    <section
      class="confirmation-window"
      role="alertdialog"
      aria-modal="true"
      :aria-label="title"
    >
      <header class="confirmation-titlebar">
        <span class="confirmation-brand">FM-MATE</span>
        <strong>{{ title }}</strong>
        <button type="button" aria-label="Close dialog" @click="emit('cancel')">
          ×
        </button>
      </header>
      <div class="confirmation-content">
        <p>{{ message }}</p>
        <footer>
          <button
            type="button"
            :class="
              cancelVariant === 'danger'
                ? 'confirmation-danger'
                : 'outline-button'
            "
            @click="emit('secondary')"
          >
            {{ cancelLabel }}
          </button>
          <button
            type="button"
            :class="
              confirmVariant === 'danger'
                ? 'confirmation-danger'
                : 'action-button action-button--primary'
            "
            @click="emit('confirm')"
          >
            {{ confirmLabel }}
          </button>
        </footer>
      </div>
    </section>
  </div>
</template>

<style>
.confirmation-backdrop {
  position: fixed;
  z-index: 70;
  inset: 0;
  display: grid;
  place-items: center;
  overflow: auto;
  padding: 20px;
  background: rgb(0 0 0 / 72%);
}
.confirmation-window {
  width: min(480px, 100%);
  color: #e5dfd5;
  background: #292726;
  border: 1px solid #65586a;
  box-shadow: 0 18px 60px rgb(0 0 0 / 65%);
}
.confirmation-titlebar {
  min-height: 43px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 9px 6px 13px;
  background: linear-gradient(90deg, #211f1e, #302b30);
  border-bottom: 1px solid #514754;
}
.confirmation-brand {
  color: #72e7c3;
  font:
    italic 800 13px Arial,
    sans-serif;
  letter-spacing: -0.04em;
}
.confirmation-titlebar strong {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: #c9b8df;
  font-size: 11px;
  letter-spacing: 0.13em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.confirmation-titlebar button {
  width: 29px;
  height: 29px;
  color: #d5c8d7;
  background: #382e35;
  border: 1px solid #665364;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}
.confirmation-titlebar button:hover {
  color: #fff;
  background: #553b43;
}
.confirmation-content {
  padding: 20px;
}
.confirmation-content p {
  margin: 0 0 22px;
  color: #d5cec6;
  font:
    13px/1.6 "Share Tech Mono",
    monospace;
  overflow-wrap: anywhere;
}
.confirmation-content footer {
  display: flex;
  justify-content: flex-end;
  gap: 9px;
}
.confirmation-content footer button {
  min-height: 38px;
  padding: 7px 12px;
  font-size: 11px;
  cursor: pointer;
}
.confirmation-danger {
  color: #ffe2df;
  background: #4a2a28;
  border: 1px solid #b56b63;
}
.confirmation-danger:hover {
  background: #60332f;
  border-color: #d68177;
}
</style>
