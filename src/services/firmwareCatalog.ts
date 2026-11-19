import { LazyStore } from "@tauri-apps/plugin-store";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  DEFAULT_FIRMWARE_REPOSITORY_URL,
  FIRMWARE_CATALOG_REFRESHED_EVENT,
} from "../config";


export interface FirmwareRecord {
  id: string;
  name: string;
  kind?: string;
  author?: string;
  repo?: string;
  ident?: string;
  what?: string;
  product?: string;
  version: string;
  date?: string;
  added?: string;
  size?: number;
  pkg: string;
  sha256: string;
  card?: string | null;
  stars?: number;
  installs?: number;
  lic?: string;
  release?: string;
  site?: string;
  play?: string;
  beta?: boolean;
  editor?: string;
}

export interface ScannedFirmware {
  id: string;
  name: string;
  version: string;
  kind: string;
  description: string;
  author: string;
  fileName: string;
  path: string;
  downloadedAt: string;
  savesPath: string;
  savesCount: number;
  saves: string[];
  currentVersion?: string | null;
  thumbnailData?: string | null;
  info: Record<string, unknown>;
  versionInfo?: Record<string, unknown> | null;
  editorPath?: string | null;
  sha256?: string | null;
  pkg: string;
}


export interface DownloadedFirmware {
  id: string;
  name: string;
  version: string;
  kind: string;
  fileName: string;
  path: string;
  downloadedAt: string;
  sha256?: string | null;
}


export interface FirmwareCatalog {
  updated: string;
  repositoryUrl: string;
  license?: FirmwareCatalogLicense | null;
  firmwares: FirmwareRecord[];
  fetchedAt: number;
  stale: boolean;
}

export interface FirmwareCatalogLicense {
  license?: string;
  url?: string;
  attribution?: string;
}

interface FirmwareCatalogCache {
  updated?: string;
  repositoryUrl?: string;
  license?: FirmwareCatalogLicense | null;
  firmwares?: FirmwareRecord[];
  fetchedAt?: number;
}

const CACHE_KEY = "firmwareCatalogCache";
const CACHE_TTL_MS = 24 * 60 * 60 * 1000;
const store = new LazyStore("settings.json", { autoSave: true });

export async function clearFirmwareCatalogCache() {
  if (isTauri()) {
    await store.delete(CACHE_KEY);
    await store.save();
  } else {
    localStorage.removeItem(`fm-mate.${CACHE_KEY}`);
  }
}

export function isNewerFirmwareVersion(
  firmware: FirmwareRecord,
  downloaded: DownloadedFirmware[],
): boolean {
  const installed = downloaded.filter((download) => download.id === firmware.id);
  return (
    installed.length > 0 &&
    !installed.some((download) =>
      firmware.sha256 && download.sha256
        ? firmware.sha256.toLowerCase() === download.sha256.toLowerCase()
        : Boolean(firmware.version && firmware.version === download.version),
    )
  );
}

export function findFirmwareUpdate(
  firmwareId: string,
  catalog: FirmwareRecord[],
  downloaded: DownloadedFirmware[],
): FirmwareRecord | null {
  const latest = catalog
    .filter((item) => item.id === firmwareId && item.version?.trim())
    .sort((a, b) =>
      b.version.localeCompare(a.version, undefined, {
        numeric: true,
        sensitivity: "base",
      }),
    )[0];
  return latest && isNewerFirmwareVersion(latest, downloaded) ? latest : null;
}

export function findConnectedFirmwareVersion<
  T extends {
    id: string;
    version: string;
    versionInfo?: Record<string, unknown> | null;
  },
>(deviceName: string, installed: T[]): T | null {
  const matches = installed.filter((firmware) => {
    const ident = firmware.versionInfo?.ident;
    const product = firmware.versionInfo?.product;
    if (
      typeof ident !== "string" ||
      !ident.trim() ||
      typeof product !== "string" ||
      !product.trim()
    ) return false;
    let pattern: RegExp;
    try {
      pattern = new RegExp(`^(?:${ident})$`);
    } catch (error) {
      console.warn(`Invalid firmware ident for ${firmware.id}:`, error);
      return false;
    }
    if (!pattern.test(product)) return false;
    const escapedProduct = product.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(
      `(?:^|[^a-zA-Z0-9_])${escapedProduct}(?=$|[^a-zA-Z0-9_])`,
    ).test(deviceName);
  });
  return matches.length === 1 ? matches[0] : null;
}

export async function getInstalledFirmwares() {
  return await invoke<ScannedFirmware[]>(
    "scan_installed_firmwares",
  );

}

export async function getFavoriteFirmwareIds() {
  return (await store.get<string[]>("favoriteFirmwareIds")) ?? [];
}


export async function getRepositoryUrl() {
  if (isTauri()) {
    const saved = await store.get<{ firmwareRepositoryUrl?: string }>(
      "preferences",
    );
    return (
      saved?.firmwareRepositoryUrl?.trim() || DEFAULT_FIRMWARE_REPOSITORY_URL
    );
  }
  const saved = localStorage.getItem("fm-mate.preferences");
  if (!saved) return DEFAULT_FIRMWARE_REPOSITORY_URL;
  try {
    return (
      (
        JSON.parse(saved) as { firmwareRepositoryUrl?: string }
      ).firmwareRepositoryUrl?.trim() || DEFAULT_FIRMWARE_REPOSITORY_URL
    );
  } catch {
    return DEFAULT_FIRMWARE_REPOSITORY_URL;
  }
}

export function resolveRepositoryUrl(path: string, repositoryUrl: string) {
  const url = repositoryUrl.replace("/catalog.json", "/") + path;
  return url;
}

export function firmwareThumbnailUrls(
  id: string,
  card: string | null | undefined,
  repositoryUrl: string,
) {
  const fallback = resolveRepositoryUrl(
    `/img/w0/${encodeURIComponent(id)}.webp`,
    repositoryUrl,
  );
  return card?.trim()
    ? [resolveRepositoryUrl(card, repositoryUrl), fallback]
    : [fallback];
}

async function getCache(): Promise<FirmwareCatalogCache | null> {
  try {
    if (isTauri())
      return (await store.get<FirmwareCatalogCache>(CACHE_KEY)) ?? null;
    const value = localStorage.getItem(`fm-mate.${CACHE_KEY}`);
    return value ? (JSON.parse(value) as FirmwareCatalogCache) : null;
  } catch {
    return null;
  }
}

async function setCache(cache: FirmwareCatalogCache) {
  if (isTauri()) {
    await store.set(CACHE_KEY, cache);
    await store.save();
  } else {
    localStorage.setItem(`fm-mate.${CACHE_KEY}`, JSON.stringify(cache));
  }
}

function toCatalog(
  cache: FirmwareCatalogCache,
  repositoryUrl: string,
  stale: boolean,
): FirmwareCatalog {
  return {
    updated: typeof cache.updated === "string" ? cache.updated : "",
    repositoryUrl,
    license: cache.license,
    firmwares: cache.firmwares ?? [],
    fetchedAt: cache.fetchedAt ?? 0,
    stale,
  };
}

export async function loadFirmwareCatalog(
  signal?: AbortSignal,
): Promise<FirmwareCatalog> {
  const repositoryUrl = await getRepositoryUrl();
  const cache = await getCache();
  const matchingCache =
    cache?.repositoryUrl === repositoryUrl &&
    Array.isArray(cache.firmwares) &&
    typeof cache.fetchedAt === "number"
      ? cache
      : null;

  if (
    matchingCache &&
    Object.prototype.hasOwnProperty.call(matchingCache, "license") &&
    typeof matchingCache.fetchedAt === "number" &&
    Date.now() - matchingCache.fetchedAt < CACHE_TTL_MS
  ) {
    return toCatalog(matchingCache, repositoryUrl, false);
  }

  try {
    const response = await fetch(repositoryUrl, {
      signal,
      headers: { Accept: "application/json" },
    });
    if (!response.ok)
      throw new Error(`Repository request failed (${response.status}).`);
    const manifest = (await response.json()) as {
      updated?: unknown;
      license?: unknown;
      firmwares?: unknown;
    };
    if (!Array.isArray(manifest.firmwares))
      throw new Error("The repository JSON must contain a firmwares array.");
    const firmwares = manifest.firmwares.filter(
      (item): item is FirmwareRecord =>
        typeof item?.id === "string" && typeof item?.name === "string",
    );
    const license =
      manifest.license &&
      typeof manifest.license === "object" &&
      !Array.isArray(manifest.license)
        ? (Object.fromEntries(
            Object.entries(manifest.license).filter(
              (entry): entry is [string, string] =>
                typeof entry[1] === "string",
            ),
          ) as FirmwareCatalogLicense)
        : null;
    const fresh: FirmwareCatalogCache = {
      updated: typeof manifest.updated === "string" ? manifest.updated : "",
      repositoryUrl,
      license,
      firmwares,
      fetchedAt: Date.now(),
    };
    await setCache(fresh);
    const catalog = toCatalog(fresh, repositoryUrl, false);
    window.dispatchEvent(
      new CustomEvent<FirmwareCatalog>(FIRMWARE_CATALOG_REFRESHED_EVENT, {
        detail: catalog,
      }),
    );
    return catalog;
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") throw error;
    if (matchingCache) return toCatalog(matchingCache, repositoryUrl, true);
    throw error;
  }
}
