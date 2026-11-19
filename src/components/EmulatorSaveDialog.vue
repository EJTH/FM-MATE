<script setup lang="ts">
import { ref, watch } from "vue";
import {
  completeEmulatorSession,
  currentEmulatorSession,
} from "../services/emulatorSession";
import PromptDialog from "./PromptDialog.vue";

const name = ref("");
const errorMessage = ref("");
const busy = ref(false);

watch(currentEmulatorSession, (session) => {
  name.value = session?.suggestedName ?? "";
  errorMessage.value = "";
});

async function finish(save: boolean) {
  if (busy.value) return;
  busy.value = true;
  errorMessage.value = "";
  try {
    await completeEmulatorSession(save ? name.value : null);
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <PromptDialog
    v-if="currentEmulatorSession"
    v-model="name"
    title="SAVE EMULATOR SESSION"
    label="Save emulator session as:"
    confirm-label="SAVE"
    cancel-label="DISCARD"
    :error-message="errorMessage"
    :disabled="busy"
    @submit="finish(true)"
    @cancel="finish(false)"
  />
</template>
