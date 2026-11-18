// cargoLockDeny.mjs — the deny-list `lint:locks` holds against every Cargo.lock: a crate that must not be pinned at all.
// What a reader gets wrong: an entry is enforced only while `enabled` is true.

// QuickJS left every host with step 5's A4: no lock may pin it again.
const QUICKJS_GONE = true;

export const DENY = [
  { name: "rquickjs", enabled: QUICKJS_GONE, why: "QuickJS is gone" },
  { name: "rquickjs-sys", enabled: QUICKJS_GONE, why: "QuickJS is gone" },
];

/** The package names a Cargo.lock pins. */
export function pinned(lockText) {
  return [...lockText.matchAll(/^name = "([^"]+)"$/gm)].map((m) => m[1]);
}

/** The enabled entries of `deny` that `lockText` pins. */
export function denied(lockText, deny = DENY) {
  const names = new Set(pinned(lockText));
  return deny.filter((d) => d.enabled && names.has(d.name));
}
