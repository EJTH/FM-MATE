<script setup lang="ts">
defineProps<{
  name: string;
  description: string;
  repository: string;
  installed: boolean;
  version?: string | null;
  busy?: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{ action: [] }>();
</script>

<template>
  <article class="tool-download-card">
    <div class="tool-download-card__content">
      <h3>{{ name }}</h3>
      <p>{{ description }}</p>
      <a
        :href="`https://github.com/${repository}`"
        target="_blank"
        rel="noreferrer"
        >https://github.com/{{ repository }} ↗</a
      >
      <small v-if="installed && version">Installed {{ version }}</small>
    </div>
    <button
      class="outline-button tool-download-card__button"
      type="button"
      :disabled="busy || disabled"
      @click="emit('action')"
    >
      {{
        busy
          ? "DOWNLOADING / CHECKING…"
          : installed
            ? "Check for Update"
            : "Download"
      }}
    </button>
  </article>
</template>

<style>
.tool-download-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 15px 0;
  border-top: 1px solid rgb(81 71 84 / 55%);
}
.tool-download-card__content {
  min-width: 0;
}
.tool-download-card h3 {
  margin: 0 0 3px;
  color: #e3dcd2;
  font-size: 16px;
  font-weight: 700;
}
.tool-download-card p {
  margin: 0 0 5px;
  color: #a49b97;
  font:
    12px/1.4 "Share Tech Mono",
    monospace;
}
.tool-download-card a {
  color: #a995bf;
  font:
    11px "Share Tech Mono",
    monospace;
  overflow-wrap: anywhere;
}
.tool-download-card small {
  display: block;
  margin-top: 5px;
  color: #72e7c3;
  font:
    10px "Share Tech Mono",
    monospace;
}
.tool-download-card__button {
  flex: 0 0 auto;
  min-width: 150px;
}
.tool-download-card__button:disabled {
  opacity: 0.55;
  cursor: wait;
}
@media (max-width: 450px) {
  .tool-download-card {
    align-items: flex-start;
    flex-direction: column;
  }
  .tool-download-card__button {
    width: 100%;
  }
}
</style>
