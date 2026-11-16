export interface InstalledVersion {
  version: string
  editorPath?: string | null
  path: string
  savesPath: string
  savesCount: number
  saves: string[]
  downloadedAt: string
}

export interface InstalledFirmware {
  id: string
  name: string
  kind: string
  description: string
  author: string
  currentVersion?: string | null
  thumbnailData?: string | null
  installedVersions: InstalledVersion[]
  savesCount: number
  info: Record<string, unknown>
}
