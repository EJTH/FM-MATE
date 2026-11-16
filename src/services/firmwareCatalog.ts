import { LazyStore } from '@tauri-apps/plugin-store'
import { isTauri } from '@tauri-apps/api/core'
import { DEFAULT_FIRMWARE_REPOSITORY_URL } from '../config'

export interface FirmwareRecord {
  id: string
  name: string
  kind?: string
  author?: string
  repo?: string
  ident?: string
  what?: string
  product?: string
  version?: string
  date?: string
  added?: string
  size?: number
  pkg?: string
  sha256?: string
  card?: string | null
  stars?: number
  installs?: number
  lic?: string
  release?: string
  site?: string
  play?: string
  beta?: boolean
  [key: string]: unknown
}

export interface FirmwareCatalog {
  updated: string
  repositoryUrl: string
  firmwares: FirmwareRecord[]
  fetchedAt: number
  stale: boolean
}

interface FirmwareCatalogCache {
  updated?: string
  repositoryUrl?: string
  firmwares?: FirmwareRecord[]
  fetchedAt?: number
}

const CACHE_KEY = 'firmwareCatalogCache'
const CACHE_TTL_MS = 8 * 60 * 60 * 1000
const store = new LazyStore('settings.json', { autoSave: true })

async function getRepositoryUrl() {
  if (isTauri()) {
    const saved = await store.get<{ firmwareRepositoryUrl?: string }>('preferences')
    return saved?.firmwareRepositoryUrl?.trim() || DEFAULT_FIRMWARE_REPOSITORY_URL
  }
  const saved = localStorage.getItem('fm-mate.preferences')
  if (!saved) return DEFAULT_FIRMWARE_REPOSITORY_URL
  try {
    return (JSON.parse(saved) as { firmwareRepositoryUrl?: string }).firmwareRepositoryUrl?.trim() || DEFAULT_FIRMWARE_REPOSITORY_URL
  } catch {
    return DEFAULT_FIRMWARE_REPOSITORY_URL
  }
}

async function getCache(): Promise<FirmwareCatalogCache | null> {
  try {
    if (isTauri()) return (await store.get<FirmwareCatalogCache>(CACHE_KEY)) ?? null
    const value = localStorage.getItem(`fm-mate.${CACHE_KEY}`)
    return value ? JSON.parse(value) as FirmwareCatalogCache : null
  } catch {
    return null
  }
}

async function setCache(cache: FirmwareCatalogCache) {
  if (isTauri()) {
    await store.set(CACHE_KEY, cache)
    await store.save()
  } else {
    localStorage.setItem(`fm-mate.${CACHE_KEY}`, JSON.stringify(cache))
  }
}

function toCatalog(cache: FirmwareCatalogCache, repositoryUrl: string, stale: boolean): FirmwareCatalog {
  return {
    updated: typeof cache.updated === 'string' ? cache.updated : '',
    repositoryUrl,
    firmwares: cache.firmwares ?? [],
    fetchedAt: cache.fetchedAt ?? 0,
    stale,
  }
}

export async function loadFirmwareCatalog(signal?: AbortSignal): Promise<FirmwareCatalog> {
  const repositoryUrl = await getRepositoryUrl()
  const cache = await getCache()
  const matchingCache = cache?.repositoryUrl === repositoryUrl
    && Array.isArray(cache.firmwares)
    && typeof cache.fetchedAt === 'number'
    ? cache
    : null

  if (matchingCache && typeof matchingCache.fetchedAt === 'number' && Date.now() - matchingCache.fetchedAt < CACHE_TTL_MS) {
    return toCatalog(matchingCache, repositoryUrl, false)
  }

  try {
    const response = await fetch(repositoryUrl, { signal, headers: { Accept: 'application/json' } })
    if (!response.ok) throw new Error(`Repository request failed (${response.status}).`)
    const manifest = await response.json() as { updated?: unknown; firmwares?: unknown }
    if (!Array.isArray(manifest.firmwares)) throw new Error('The repository JSON must contain a firmwares array.')
    const firmwares = manifest.firmwares.filter((item): item is FirmwareRecord =>
      typeof item?.id === 'string' && typeof item?.name === 'string',
    )
    const fresh: FirmwareCatalogCache = {
      updated: typeof manifest.updated === 'string' ? manifest.updated : '',
      repositoryUrl,
      firmwares,
      fetchedAt: Date.now(),
    }
    await setCache(fresh)
    return toCatalog(fresh, repositoryUrl, false)
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') throw error
    if (matchingCache) return toCatalog(matchingCache, repositoryUrl, true)
    throw error
  }
}
