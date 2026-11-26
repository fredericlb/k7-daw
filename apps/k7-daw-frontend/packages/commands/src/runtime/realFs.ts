// realFs.ts — the machine's own `node:fs`, for the few places that must read the disk while the runtime's shims stand in for it.
// What a reader gets wrong: `import "node:fs"` here would be the shim under the runtime's bundle and aliases; this is the real one.

type Fs = typeof import("node:fs");

export function realFs(): Fs {
  const get = (
    process as unknown as { getBuiltinModule?: (id: string) => unknown }
  ).getBuiltinModule;
  if (!get)
    throw new Error(
      "this node has no process.getBuiltinModule (node 22.3 or later)",
    );
  return get("node:fs") as Fs;
}
