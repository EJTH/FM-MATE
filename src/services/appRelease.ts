import { invoke, isTauri } from "@tauri-apps/api/core";

export interface AppUpdateInfo {
  isUpdateAvailable: boolean;
  currentVersion: string;
  latestVersion: string;
}

export interface AppRelease {
  name: string | null;
  tagName: string;
  body: string | null;
  htmlUrl: string | null;
  updateInfo: AppUpdateInfo | null;
}

let releasePromise: Promise<AppRelease> | undefined;

async function loadAppRelease(): Promise<AppRelease> {
  if (isTauri()) {
    const result = await invoke<AppUpdateInfo & {
      name: string | null;
      body: string | null;
      htmlUrl: string | null;
    }>("check_app_update");
    return {
      name: result.name,
      tagName: result.latestVersion,
      body: result.body,
      htmlUrl: result.htmlUrl,
      updateInfo: result,
    };
  }

  const response = await fetch(
    "https://api.github.com/repos/EJTH/FM-MATE/releases/latest",
    { headers: { Accept: "application/vnd.github+json" } },
  );
  if (!response.ok)
    throw new Error(`GitHub returned HTTP ${response.status}.`);
  const release: unknown = await response.json();
  if (
    !release ||
    typeof release !== "object" ||
    !("tag_name" in release) ||
    typeof release.tag_name !== "string"
  )
    throw new Error("Could not read GitHub release.");

  return {
    tagName: release.tag_name,
    name: "name" in release && typeof release.name === "string" ? release.name : null,
    body: "body" in release && typeof release.body === "string" ? release.body : null,
    htmlUrl:
      "html_url" in release && typeof release.html_url === "string"
        ? release.html_url
        : null,
    updateInfo: null,
  };
}

export function getAppRelease(): Promise<AppRelease> {
  if (!releasePromise) releasePromise = loadAppRelease();
  return releasePromise;
}
