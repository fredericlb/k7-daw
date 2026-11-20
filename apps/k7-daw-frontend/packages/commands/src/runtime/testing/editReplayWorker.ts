// editReplayWorker.ts — the edit replay as a Worker entry: given the plan, runs it through `edit.call` over the Vfs and answers the tally.
// What a reader gets wrong: it is built by `bundleRuntime` like the verbs' Worker, so the module, the shims and the aliases are the shipped ones.

import "../prelude.ts";
import { editReady } from "@studio/k7-wasm/edit";
import {
  type Backend,
  native,
  type Plan,
  replay,
} from "@studio/k7-wasm/replayRun";
import { setVfs } from "../shims/current.ts";
import { Vfs } from "../vfs.ts";

let vfs: Vfs;
const backend: Backend = {
  map: (s) => s,
  prepare(files, dirs) {
    vfs = new Vfs(true);
    setVfs(vfs);
    for (const d of dirs) vfs.mkdir(d, true);
    for (const [p, text] of Object.entries(files)) {
      vfs.mkdir(p.slice(0, p.lastIndexOf("/")) || "/", true);
      vfs.load(p, { text, len: text.length });
    }
  },
  files() {
    const out: Record<string, string> = {};
    for (const [p, rec] of vfs.files)
      if (rec.text !== undefined) out[p] = rec.text;
    return out;
  },
};

self.onmessage = async (e: MessageEvent<{ plan: Plan }>) => {
  try {
    if (!(await editReady())) throw new Error("the module is not available");
    const t = replay(e.data.plan, backend);
    const n = native(e.data.plan);
    self.postMessage({ ok: true, tally: t, native: n });
  } catch (err) {
    self.postMessage({ ok: false, error: String(err) });
  }
};
