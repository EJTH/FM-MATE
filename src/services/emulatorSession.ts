import { invoke } from '@tauri-apps/api/core'
import { shallowRef } from 'vue'

export interface EmulatorSavePrompt {
  sessionId: string
  suggestedName: string
}

interface SavedEmulatorSession {
  fileName: string
  warning: string | null
}

interface PendingSession {
  session: EmulatorSavePrompt
  resolve: (result: SavedEmulatorSession | null) => void
}

const pendingSessions: PendingSession[] = []
export const currentEmulatorSession = shallowRef<EmulatorSavePrompt | null>(null)

export function finishEmulatorSession(session: EmulatorSavePrompt): Promise<SavedEmulatorSession | null> {
  return new Promise((resolve) => {
    pendingSessions.push({ session, resolve })
    if (!currentEmulatorSession.value) currentEmulatorSession.value = session
  })
}

export async function completeEmulatorSession(name: string | null): Promise<void> {
  const pending = pendingSessions[0]
  if (!pending) throw new Error('Emulator session not found.')
  const result = await invoke<SavedEmulatorSession | null>('finish_emulator_session', {
    sessionId: pending.session.sessionId,
    name,
  })
  pendingSessions.shift()
  currentEmulatorSession.value = pendingSessions[0]?.session ?? null
  pending.resolve(result)
}
