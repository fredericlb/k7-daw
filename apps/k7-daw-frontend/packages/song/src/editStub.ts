// editStub.ts — the one door a stubbed export of song's write side goes through: a call into the Rust crate over WebAssembly.
// A reader would get wrong the synchrony: in node the module compiles at import, in the Worker `editReady()` has been awaited at open.

import { isAbsolute } from "node:path";
import { editBatch, editCallEnv, editReady } from "@studio/k7-wasm/edit";
import { injectedRootsNow } from "./injectedRoots";
import { modelsRoot } from "./modelCache";
import {
  accountLibraries,
  librariesHome,
  libraryOverlays,
  metaModulesRoot,
} from "./sharedFolders";
import { studioHome } from "./studioHome";

// Starting the load here compiles the module synchronously in node; a refusal surfaces at the first call.
void editReady().catch(() => undefined);

/** The environment the TypeScript read from the process, as the crate takes it: data, never ambient. */
export function editEnv(from?: string): Record<string, unknown> {
  const env: Record<string, unknown> = {
    studioHome: studioHome(),
    // A host that handed its roots in (the Worker) has no `@meta` folder of its own to name when it handed none.
    metaRoot:
      injectedRootsNow() && !injectedRootsNow()?.meta ? "" : metaModulesRoot(),
    librariesHome: librariesHome(),
    modelsRoot: modelsRoot(),
    overlays: libraryOverlays().map((o) => [o.name, o.root]),
  };
  if (from)
    env.libraryRoots = accountLibraries(from).map((l) => [l.name, l.root]);
  return env;
}

/** An argument as the crate's tap recorded it: what JSON cannot say is a tagged object. */
function toWire(v: unknown): unknown {
  if (v === undefined) return { $: "undefined" };
  if (typeof v === "number" && !Number.isFinite(v))
    return { $: "number", v: String(v) };
  return v;
}

/** An answer with its tagged objects turned back into the values they stand for. */
function fromWire(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(fromWire);
  if (v === null || typeof v !== "object") return v;
  const o = v as Record<string, unknown>;
  switch (o.$) {
    case "undefined":
      return undefined;
    case "number":
      return Number(o.v);
    case "bigint":
      return BigInt(o.v as string);
    case "map":
      return new Map(
        (o.v as [unknown, unknown][]).map(([k, x]) => [
          fromWire(k),
          fromWire(x),
        ]),
      );
    case "set":
      return new Set((o.v as unknown[]).map(fromWire));
  }
  return Object.fromEntries(
    Object.entries(o).map(([k, x]) => [k, fromWire(x)]),
  );
}

/** Runs `fn` (`song/preferences.setEditorVim`) in the crate with the TypeScript's own arguments; throws what the TypeScript threw. */
export function stub<R>(fn: string, ...args: unknown[]): R {
  const first = args[0];
  const from =
    typeof first === "string" && isAbsolute(first) ? first : undefined;
  return fromWire(editCallEnv(editEnv(from), fn, args.map(toWire))) as R;
}

/** A loop over a selection as ONE crossing: the calls run in order and the first throw ends the batch. */
export function stubBatch(
  calls: [fn: string, ...args: unknown[]][],
): unknown[] {
  const first = calls[0]?.[1];
  const from =
    typeof first === "string" && isAbsolute(first) ? first : undefined;
  const done = editBatch(
    calls.map(([fn, ...args]) => ({ fn, args: args.map(toWire) })),
    editEnv(from),
  );
  if (done.thrown !== undefined) throw new Error(done.thrown);
  return done.rets.map(fromWire);
}
