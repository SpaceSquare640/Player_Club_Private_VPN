/**
 * Bundles a virtual network's three join fields (host address, name,
 * password) into one copy/paste-able string, so sharing an invite is one
 * clipboard round trip instead of three — the closest honest equivalent to
 * the old project's "give a friend the Admin IP and go," without changing
 * what the join protocol actually requires (the joiner still needs all
 * three; this just avoids three separate copy/paste actions to get them).
 *
 * Plain delimited text, not base64: this only ever round-trips through this
 * app's own clipboard, never gets typed by a human, so there's nothing to
 * gain from an unreadable encoding — and it sidesteps `btoa`'s non-Latin1
 * pitfalls for Chinese network names/passwords.
 */
const PREFIX = "pcpv-invite:v1:";

export interface NetworkInvite {
  networkName: string;
  password: string;
  hostAddr: string;
}

export function encodeInvite({ networkName, password, hostAddr }: NetworkInvite): string {
  return `${PREFIX}${encodeURIComponent(networkName)}:${encodeURIComponent(password)}:${encodeURIComponent(hostAddr)}`;
}

/** `null` for anything that isn't a well-formed invite — a paste of unrelated clipboard content just fails silently rather than corrupting the form. */
export function decodeInvite(s: string): NetworkInvite | null {
  const trimmed = s.trim();
  if (!trimmed.startsWith(PREFIX)) return null;
  const parts = trimmed.slice(PREFIX.length).split(":");
  if (parts.length !== 3) return null;
  try {
    const [networkName, password, hostAddr] = parts.map(decodeURIComponent);
    if (!networkName || !password || !hostAddr) return null;
    return { networkName, password, hostAddr };
  } catch {
    return null; // malformed percent-encoding
  }
}
