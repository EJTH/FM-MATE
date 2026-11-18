import { invoke } from '@tauri-apps/api/core'

export interface EmulatorSavePrompt {
  sessionId: string
  suggestedName: string
}

interface SavedEmulatorSession {
  fileName: string
  warning: string | null
}

export async function finishEmulatorSession(session: EmulatorSavePrompt): Promise<SavedEmulatorSession | null> {
  let message = 'Save emulator session as:'
  let suggestedName = session.suggestedName
  while (true) {
    const name = window.prompt(message, suggestedName)
    if (name === null) {
      await invoke('finish_emulator_session', { sessionId: session.sessionId, name: null })
      return null
    }
    suggestedName = name
    try {
      return await invoke<SavedEmulatorSession>('finish_emulator_session', { sessionId: session.sessionId, name })
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error)
      message = `${detail}\n\nSave emulator session as (or cancel to discard):`
    }
  }
}
