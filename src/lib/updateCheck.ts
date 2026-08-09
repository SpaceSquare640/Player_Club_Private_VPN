/**
 * Check-and-notify update check — no auto-download/install. Fetches the
 * latest GitHub Release, compares it against the running app's version, and
 * returns where to go if a newer one exists. The user always does the
 * actual download/install themselves via the browser.
 */
import { fetch } from "@tauri-apps/plugin-http";
import { getVersion } from "@tauri-apps/api/app";

const LATEST_RELEASE_API =
  "https://api.github.com/repos/SpaceSquare640/Player_Club_Private_VPN/releases/latest";

export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
}

/** Numeric `major.minor.patch` compare. Returns true if `a` is newer than `b`. */
export function isNewerVersion(a: string, b: string): boolean {
  const pa = a.split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const da = pa[i] ?? 0;
    const db = pb[i] ?? 0;
    if (da !== db) return da > db;
  }
  return false;
}

/**
 * Resolves to `UpdateInfo` when a newer release exists, `null` when already
 * up to date, and rejects on any network/parse failure — callers treat that
 * as "couldn't check right now" and stay silent, the same best-effort
 * pattern the rest of the app uses for non-critical background checks.
 */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  const currentVersion = await getVersion();
  const res = await fetch(LATEST_RELEASE_API, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!res.ok) {
    throw new Error(`GitHub API returned ${res.status}`);
  }
  const data = (await res.json()) as { tag_name?: string; html_url?: string };
  if (!data.tag_name || !data.html_url) {
    throw new Error("unexpected GitHub API response shape");
  }
  const latestVersion = data.tag_name.replace(/^v/, "");
  if (!isNewerVersion(latestVersion, currentVersion)) {
    return null;
  }
  return { currentVersion, latestVersion, releaseUrl: data.html_url };
}
