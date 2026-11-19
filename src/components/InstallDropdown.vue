<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";

withDefaults(
  defineProps<{
    versions: string[];
    label?: string;
  }>(),
  { label: "INSTALL" },
);

const emit = defineEmits<{ select: [version: string] }>();
const root = ref<HTMLElement | null>(null);
const expanded = ref(false);

function closeWhenOutside(event: PointerEvent) {
  if (event.target instanceof Node && !root.value?.contains(event.target))
    expanded.value = false;
}

onMounted(() => document.addEventListener("pointerdown", closeWhenOutside));
onBeforeUnmount(() =>
  document.removeEventListener("pointerdown", closeWhenOutside),
);

function selectVersion(version: string) {
  expanded.value = false;
  emit("select", version);
}
</script>

<template>
  <div
    ref="root"
    class="install-dropdown"
    @click.stop
    @keydown.esc="expanded = false"
  >
    <button
      class="outline-button install-button"
      type="button"
      aria-haspopup="menu"
      :aria-expanded="expanded"
      :disabled="!versions.length"
      @click="expanded = !expanded"
    >
      {{ label }} <span aria-hidden="true">▾</span>
    </button>
    <div v-if="expanded" class="install-menu" role="menu">
      <button
        v-for="version in versions"
        :key="version"
        type="button"
        role="menuitem"
        @click="selectVersion(version)"
      >
        Install {{ version }}
      </button>
    </div>
  </div>
</template>

<style>
.install-dropdown {
  position: relative;
  min-width: 0;
}
.install-dropdown > .install-button {
  display: flex;
  width: 100px;
  min-width: 0;
  align-items: center;
  justify-content: center;
  gap: 7px;
  color: #c8e8df;
  background: #21413e;
  border-color: #3c9c8f;
}
.install-dropdown > .install-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.install-menu {
  position: absolute;
  z-index: 25;
  right: 0;
  bottom: calc(100% + 6px);
  left: 0;
  display: grid;
  max-height: 220px;
  overflow: auto;
  padding: 4px;
  background: #242221;
  border: 1px solid #665772;
  box-shadow: 0 8px 20px #0008;
}
.install-menu button {
  padding: 8px 9px;
  color: #d8d0c8;
  background: transparent;
  border: 0;
  font:
    12px "Share Tech Mono",
    monospace;
  text-align: left;
  cursor: pointer;
}
.install-menu button:hover,
.install-menu button:focus-visible {
  color: #fff;
  background: #34302f;
}
</style>
