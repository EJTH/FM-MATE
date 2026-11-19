import { reactive } from "vue";

export interface ArchiveCommit {
  sha: string;
  date: string;
}

interface MirrorRepository {
  owner: string;
  repo: string;
  ref: string;
  path: string;
}

export function parseMirrorRepository(value: string): MirrorRepository | null {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return null;
  }
  if (
    (url.protocol !== "https:" && url.protocol !== "http:") ||
    url.hostname !== "raw.githubusercontent.com" ||
    !url.pathname.includes("mirror")
  ) return null;

  const parts = url.pathname.slice(1).split("/");
  const refLength = parts[2] === "refs" && ["heads", "tags"].includes(parts[3] ?? "") ? 3 : 1;
  if (parts.length <= 2 + refLength || parts.some((part) => !part)) return null;
  try {
    const [owner, repo] = parts.map(decodeURIComponent);
    return {
      owner,
      repo,
      ref: parts.slice(2, 2 + refLength).map(decodeURIComponent).join("/"),
      path: parts.slice(2 + refLength).map(decodeURIComponent).join("/"),
    };
  } catch {
    return null;
  }
}

export function archiveCatalogUrl(repositoryUrl: string, sha: string): string {
  const repository = parseMirrorRepository(repositoryUrl);
  if (!repository || !/^[a-f\d]{40}$/i.test(sha)) return repositoryUrl;
  const path = repository.path.split("/").map(encodeURIComponent).join("/");
  return `https://raw.githubusercontent.com/${encodeURIComponent(repository.owner)}/${encodeURIComponent(repository.repo)}/${sha}/${path}`;
}

export const archiveState = reactive({
  repositoryUrl: "",
  commits: [] as ArchiveCommit[],
  loading: false,
  error: "",
});

let requestId = 0;
let currentRequest: Promise<void> | undefined;

export function loadArchiveCommits(repositoryUrl: string, force = false): Promise<void> {
  if (!force && archiveState.repositoryUrl === repositoryUrl && currentRequest) return currentRequest;
  const id = ++requestId;
  archiveState.repositoryUrl = repositoryUrl;
  archiveState.commits = [];
  archiveState.error = "";
  const repository = parseMirrorRepository(repositoryUrl);
  archiveState.loading = !!repository;
  if (!repository) {
    currentRequest = undefined;
    return Promise.resolve();
  }

  const url = new URL(
    `https://api.github.com/repos/${encodeURIComponent(repository.owner)}/${encodeURIComponent(repository.repo)}/commits`,
  );
  url.searchParams.set("sha", repository.ref);
  url.searchParams.set("path", repository.path);
  url.searchParams.set("per_page", "100");
  currentRequest = (async () => {
    try {
      const response = await fetch(url, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!response.ok) throw new Error(`GitHub returned HTTP ${response.status}.`);
      const data: unknown = await response.json();
      if (!Array.isArray(data)) throw new Error("Invalid GitHub commits response.");
      const commits: ArchiveCommit[] = data.map((item: unknown) => {
        if (
          !item || typeof item !== "object" ||
          !("sha" in item) || typeof item.sha !== "string" || !/^[a-f\d]{40}$/i.test(item.sha) ||
          !("commit" in item) || !item.commit || typeof item.commit !== "object" ||
          !("committer" in item.commit) || !item.commit.committer || typeof item.commit.committer !== "object" ||
          !("date" in item.commit.committer) || typeof item.commit.committer.date !== "string" ||
          Number.isNaN(Date.parse(item.commit.committer.date))
        ) throw new Error("Invalid GitHub commit in response.");
        return { sha: item.sha, date: item.commit.committer.date };
      });
      if (id === requestId) archiveState.commits = commits;
    } catch (error) {
      if (id === requestId) {
        archiveState.error = error instanceof Error ? error.message : "Could not load archive dates.";
        console.error("Could not load firmware archive dates:", error);
      }
    } finally {
      if (id === requestId) archiveState.loading = false;
    }
  })();
  return currentRequest;
}
