<script setup lang="ts">
import ToolDownloadsSection from './ToolDownloadsSection.vue'

const emit = defineEmits<{ dismiss: [] }>()
</script>

<template>
  <div class="welcome-backdrop" role="presentation" @click.self="emit('dismiss')" @keydown.esc="emit('dismiss')" tabindex="-1">
    <section class="welcome-window" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
      <header class="welcome-titlebar">
        <span class="welcome-brand">FM-MATE</span>
        <strong id="welcome-title">WELCOME</strong>
        <button type="button" aria-label="Close welcome dialog" @click="emit('dismiss')">×</button>
      </header>
      <div class="welcome-content">
        <img src="/fm-mate-logo.svg" alt="FM-MATE Logo" class="welcome-logo" />
        <p class="welcome-thanks">Thanks for downloading FM-MATE!</p>
        <p class="welcome-intro">Before you get started, be sure </p>
        <ToolDownloadsSection
          title="TOOLS YOU CAN DOWNLOAD"
          description="M-Flasher is required to install firmware and create device backups. FM1-Emu is optional and lets you try firmware locally before using it on your device."
        />
        <footer><button class="action-button action-button--primary" type="button" @click="emit('dismiss')">GET STARTED</button></footer>
      </div>
    </section>
  </div>
</template>

<style>
.welcome-backdrop { position: fixed; z-index: 80; inset: 0; display: grid; place-items: center; overflow: auto; padding: 20px; background: rgb(0 0 0 / 76%); }
.welcome-window { width: min(760px, 100%); max-height: min(90vh, 900px); overflow: auto; color: #e5dfd5; background: #292726; border: 1px solid #65586a; box-shadow: 0 18px 60px rgb(0 0 0 / 65%); }
.welcome-titlebar { position: sticky; top: 0; min-height: 43px; display: flex; align-items: center; gap: 12px; padding: 6px 9px 6px 13px; background: linear-gradient(90deg, #211f1e, #302b30); border-bottom: 1px solid #514754; }
.welcome-brand { color: #72e7c3; font: italic 800 13px Arial, sans-serif; letter-spacing: -.04em; }
.welcome-titlebar strong { min-width: 0; flex: 1; color: #c9b8df; font-size: 11px; letter-spacing: .13em; }
.welcome-titlebar button { width: 29px; height: 29px; color: #d5c8d7; background: #382e35; border: 1px solid #665364; font-size: 20px; line-height: 1; cursor: pointer; }
.welcome-content { padding: 22px; }
.welcome-logo { width: 240px; height: 240px; margin: 0 auto 20px; display: block; object-fit: contain; }
.welcome-thanks { margin: 0 0 7px; color: #72e7c3; font-size: 22px; font-weight: 700; }
.welcome-intro { margin: 0 0 18px; color: #d5cec6; font: 13px/1.6 'Share Tech Mono', monospace; }
.welcome-content .settings-group { padding: 0; border: 0; }
.welcome-content footer { display: flex; justify-content: flex-end; margin-top: 14px; }
.welcome-content footer button { min-height: 40px; padding: 8px 18px; cursor: pointer; }
</style>
