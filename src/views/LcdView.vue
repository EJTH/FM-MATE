<script setup lang="ts">
import LcdDisplay from '../components/LcdDisplay.vue'
</script>

<template>
  <section class="view-panel lcd-view">
    <div class="view-heading"><div><p class="eyebrow">DEVICE STATUS</p><h1>LCD Display</h1></div><span class="connection-pill"><i /> WAITING FOR DEVICE</span></div>
    <div class="lcd-card"><LcdDisplay /><p class="lcd-caption">LIVE DEVICE DISPLAY <span>—</span> Updates when a compatible device is connected.</p></div>
    <div class="device-note"><span class="note-icon">⌁</span><div><strong>No device detected</strong><p>Connect an FM-1 or SMK-37 to view its live display.</p></div></div>
  </section>
</template>
