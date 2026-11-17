<script setup lang="ts">
import LcdDisplay from './LcdDisplay.vue'
import type { MFlasherLcdState } from '../types/flasher'

type View = 'browse' | 'downloaded' | 'backups' | 'settings' | 'lcd'

defineProps<{ activeView: View; lcdState: MFlasherLcdState }>()
const emit = defineEmits<{ navigate: [view: View] }>()

const views: { id: View; label: string }[] = [
  { id: 'browse', label: 'Browse' },
  { id: 'downloaded', label: 'Library' },
  { id: 'backups', label: 'Backups' },
  { id: 'settings', label: 'Settings' },
]
</script>

<template>
  <header class="topbar">
    <div class="topbar-inner">
      <a class="brand" href="#home" aria-label="FM-MATE home" @click.prevent="emit('navigate', 'browse')">
        <img src="/fm-mate-logo.svg" alt="FM-MATE" class="brand-logo" />
      </a>
      <nav class="top-nav" aria-label="Main navigation" role="tablist">
        <button
          v-for="view in views"
          :key="view.id"
          class="top-tab"
          :class="{ 'is-active': activeView === view.id }"
          type="button"
          role="tab"
          :aria-selected="activeView === view.id"
          @click="emit('navigate', view.id)"
        >
          <span class="tab-indicator" aria-hidden="true" />{{ view.label }}
        </button>
      </nav>
      <button class="header-lcd" type="button" role="tab" :aria-selected="activeView === 'lcd'" aria-label="Open LCD display" @click="emit('navigate', 'lcd')">
        <LcdDisplay v-bind="lcdState" />
      </button>
    </div>
  </header>
</template>

<style>
.topbar { position: sticky; z-index: 10; top: 0; width: 100%; background: #211f1e; border-bottom: 1px solid #514754; box-shadow: 0 8px 18px rgb(0 0 0 / 55%); }
.topbar-inner { width: 100%; max-width: 1500px; min-height: 78px; display: grid; grid-template-columns: auto minmax(0, 1fr) minmax(190px, 26%); align-items: center; gap: clamp(12px, 2vw, 28px); margin: 0 auto; padding: 12px clamp(18px, 3vw, 44px) 16px; }
.brand { flex: 0 0 auto; color: #72e7c3; font-family: Arial, sans-serif; font-size: clamp(22px, 2.4vw, 30px); font-style: italic; font-weight: 900; letter-spacing: -.065em; text-decoration: none; display: flex; align-items: center; justify-content: center; }
.brand-logo { height: 60px; width: auto; display: flex; align-items: center; }
.top-nav { display: flex; min-width: 0; align-items: center; justify-content: center; gap: 5px; }
.top-tab { display: inline-flex; min-height: 39px; align-items: center; gap: 9px; padding: 0 12px; color: #c8c0d0; background: linear-gradient(135deg, #302d2c, #262423); border: 1px solid #514b4a; font-size: 14px; font-weight: 700; letter-spacing: .04em; white-space: nowrap; cursor: pointer; transition: border-color .15s, color .15s, background .15s; }
.top-tab:hover { color: #f0e9df; border-color: #82728d; }.top-tab.is-active { color: #f0eae2; background: linear-gradient(135deg, #21413e, #1b3331); border-color: #3c9c8f; box-shadow: inset 0 0 0 1px rgb(87 224 198 / 9%); }
.tab-indicator { width: 9px; height: 9px; flex: 0 0 9px; border-radius: 50%; background: #45403e; box-shadow: inset 0 1px 2px #171514; }.is-active .tab-indicator { background: #f19a47; box-shadow: 0 0 8px rgb(241 154 71 / 45%); }
.header-lcd { display: block; width: 100%; min-width: 0; padding: 5px; overflow: hidden; background: #111110; border: 1px solid #51483e; cursor: pointer; text-align:left }
.header-lcd .lcd-bezel { width: 100%; padding: 3px; border: 0; box-shadow: none; }.header-lcd .lcd-screen { min-height: 45px; padding: 5px 7px; column-gap: 6px; grid-template-columns: minmax(22px, 24%) minmax(0, 1fr); aspect-ratio: auto; }
.header-lcd .lcd-device-icon { border-width: 2px; }.header-lcd .lcd-device-icon span { height: 2px; background: repeating-linear-gradient(90deg, currentColor 0 2px, transparent 2px 5px); }.header-lcd .lcd-content { padding-left: 6px; }
.header-lcd .lcd-content h2 { font-size: clamp(10px, 1.1vw, 14px); }.header-lcd .lcd-message, .header-lcd .lcd-connection { margin-top: 1px; font-size: clamp(7px, .8vw, 10px); }
.header-lcd .lcd-hint { margin-top: 2px; padding-top: 1px; font-size: clamp(6px, .7vw, 9px); }
.header-lcd .lcd-message { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.header-lcd .lcd-device-icon .lcd-progress-value { width: auto; height: auto; background: none; font-size: clamp(8px, .9vw, 14px); font-weight: 700; line-height: 1; }
.header-lcd:focus-visible, .top-tab:focus-visible { outline: 2px solid #65e1c1; outline-offset: 2px; }
@media (max-width: 1000px) {
  .topbar-inner { grid-template-columns: minmax(0, 1fr) minmax(190px, 34%); grid-template-areas: 'brand lcd' 'nav nav'; row-gap: 12px; }
  .brand { grid-area: brand; }.header-lcd { grid-area: lcd; max-width: 360px; justify-self: end; }
  .top-nav { grid-area: nav; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px; }
  .top-tab { justify-content: flex-start; padding-inline: 12px; font-size: 13px; }
    
}
@media (max-width: 450px) {
  .topbar-inner { grid-template-columns: minmax(0, 1fr) minmax(190px, 34%); grid-template-areas: 'brand lcd' 'nav nav'; row-gap: 12px; }
  .brand { grid-area: brand; }.header-lcd { grid-area: lcd; max-width: 360px; justify-self: end; }
  .top-nav { grid-area: nav; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; }
  .top-tab { justify-content: flex-start; padding-inline: 12px; font-size: 13px; }
}
@media (max-width: 320px) {
  .topbar-inner { grid-template-columns: minmax(0, .9fr) minmax(110px, 1.1fr); gap: 10px; padding: 10px 14px 16px; }
  .top-nav { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; }
  .top-tab { min-height: 36px; padding-inline: 10px; font-size: 12px; }
  .tab-indicator { width: 7px; height: 7px; flex-basis: 7px; }
}
</style>
