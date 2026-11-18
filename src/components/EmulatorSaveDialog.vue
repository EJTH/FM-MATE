<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { completeEmulatorSession, currentEmulatorSession } from '../services/emulatorSession'

const name = ref('')
const errorMessage = ref('')
const busy = ref(false)
const nameInput = ref<HTMLInputElement | null>(null)

watch(currentEmulatorSession, async (session) => {
  name.value = session?.suggestedName ?? ''
  errorMessage.value = ''
  if (session) {
    await nextTick()
    nameInput.value?.focus()
    nameInput.value?.select()
  }
})

async function finish(save: boolean) {
  if (busy.value) return
  busy.value = true
  errorMessage.value = ''
  try {
    await completeEmulatorSession(save ? name.value : null)
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div v-if="currentEmulatorSession" class="emulator-save-backdrop">
    <section class="emulator-save-dialog" role="dialog" aria-modal="true" aria-labelledby="emulator-save-title">
      <header class="emulator-save-titlebar">
        <span>FM-MATE</span>
        <strong id="emulator-save-title">SAVE EMULATOR SESSION</strong>
      </header>
      <form class="emulator-save-content" @submit.prevent="finish(true)">
        <label for="emulator-save-name">Save emulator session as:</label>
        <input id="emulator-save-name" ref="nameInput" v-model="name" type="text" :disabled="busy" />
        <p v-if="errorMessage" class="emulator-save-error" role="alert">{{ errorMessage }}</p>
        <footer>
          <button type="button" class="outline-button" :disabled="busy" @click="finish(false)">DISCARD</button>
          <button type="submit" class="action-button action-button--primary" :disabled="busy">SAVE</button>
        </footer>
      </form>
    </section>
  </div>
</template>

<style>
.emulator-save-backdrop { position: fixed; z-index: 100; inset: 0; display: grid; place-items: center; overflow: auto; padding: 20px; background: rgb(0 0 0 / 72%); }
.emulator-save-dialog { width: min(480px, 100%); color: #e5dfd5; background: #292726; border: 1px solid #65586a; box-shadow: 0 18px 60px rgb(0 0 0 / 65%); }
.emulator-save-titlebar { min-height: 43px; display: flex; align-items: center; gap: 12px; padding: 6px 13px; background: linear-gradient(90deg, #211f1e, #302b30); border-bottom: 1px solid #514754; }
.emulator-save-titlebar span { color: #72e7c3; font: italic 800 13px Arial, sans-serif; letter-spacing: -.04em; }
.emulator-save-titlebar strong { color: #c9b8df; font-size: 11px; letter-spacing: .13em; }
.emulator-save-content { display: grid; gap: 12px; padding: 20px; }
.emulator-save-content label, .emulator-save-error { color: #d5cec6; font: 13px/1.6 'Share Tech Mono', monospace; }
.emulator-save-content input { width: 100%; padding: 8px 10px; color: #e3dcd2; background: #211f1e; border: 1px solid #665772; font: 13px 'Share Tech Mono', monospace; }
.emulator-save-error { margin: 0; color: #efaaa3; overflow-wrap: anywhere; }
.emulator-save-content footer { display: flex; justify-content: flex-end; gap: 9px; margin-top: 10px; }
.emulator-save-content button { min-height: 38px; padding: 7px 12px; font-size: 11px; cursor: pointer; }
.emulator-save-content button:disabled { opacity: .55; cursor: wait; }
</style>
