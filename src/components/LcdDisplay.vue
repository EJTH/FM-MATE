<script setup lang="ts">
withDefaults(defineProps<{
  title?: string
  message?: string
  connection?: string
  hint?: string
}>(), {
  title: "NO UNIT",
  message: "WAITING FOR DOWNLOAD MODE",
  connection: "USB",
  hint: "HOLD SEQ REC (SMK-37) OR REC (FM-1) WHILE SWITCHING ON",
})
</script>

<template>
  <section class="lcd-bezel" aria-label="Device LCD display">
    <div class="lcd-screen">
      <slot name="graphic">
        <div class="lcd-device-icon" aria-hidden="true"><span /></div>
      </slot>
      <div class="lcd-content">
        <slot>
          <h2>{{ title }}</h2>
          <p class="lcd-message">{{ message }}</p>
          <p class="lcd-connection">{{ connection }}</p>
        </slot>
      </div>
      <p class="lcd-hint"><slot name="hint">{{ hint }}</slot></p>
    </div>
  </section>
</template>
