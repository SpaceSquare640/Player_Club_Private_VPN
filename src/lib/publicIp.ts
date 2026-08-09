/**
 * Best-effort public-IP lookup, so hosting a Relay Server doesn't require
 * the user to go find their own address on a third-party website — the
 * classic "what's my IP" step from the user's own prior relay project's
 * Admin panel, done in-app instead. Same fetch-and-fail-silently posture as
 * `updateCheck.ts`: this is a convenience, not something any connection flow
 * depends on, so a failure here is never fatal to anything else.
 */
import { fetch } from "@tauri-apps/plugin-http";

const PUBLIC_IP_API = "https://api.ipify.org";

/** Resolves to the caller's public IPv4 address, or `null` if the lookup failed (offline, API down, ...). */
export async function getPublicIp(): Promise<string | null> {
  try {
    const res = await fetch(PUBLIC_IP_API);
    if (!res.ok) return null;
    const text = (await res.text()).trim();
    return /^\d{1,3}(\.\d{1,3}){3}$/.test(text) ? text : null;
  } catch {
    return null;
  }
}
