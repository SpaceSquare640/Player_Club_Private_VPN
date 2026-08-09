/**
 * A default network name for whoever hosts, so the Create form's name field
 * doesn't have to be filled in — mirrors the old `Admin_App.py`/`Client_App.py`
 * project, where the host ("Admin") never had to name anything at all.
 * `network_name` is still protocol-load-bearing on the joiner's side (see
 * `mesh.rs`), so it can't be dropped there — this only removes the "think of
 * a name" step on the host's side.
 */
const ADJECTIVES = [
  "swift",
  "brave",
  "quiet",
  "golden",
  "lucky",
  "clever",
  "bold",
  "calm",
  "sunny",
  "misty",
  "amber",
  "crimson",
];

const NOUNS = ["fox", "hawk", "otter", "wolf", "falcon", "tiger", "raven", "lynx", "panda", "eagle", "bear", "crane"];

/** e.g. `"swift-fox-42"` — not guaranteed unique, just unlikely to collide; a taken name is rejected the same way any manually-typed one would be. */
export function generateNetworkName(): string {
  const adjective = ADJECTIVES[Math.floor(Math.random() * ADJECTIVES.length)];
  const noun = NOUNS[Math.floor(Math.random() * NOUNS.length)];
  const suffix = Math.floor(Math.random() * 100)
    .toString()
    .padStart(2, "0");
  return `${adjective}-${noun}-${suffix}`;
}
