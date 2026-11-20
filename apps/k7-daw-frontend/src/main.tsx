// main.tsx — the editor's entry: the documentation site has its own (docs.html), so nothing here is shared with it.

import { reloadForChunk, reloadOnChunkError } from "./chunkReload";

reloadOnChunkError();

// Dynamic, so a failure past the one reload is said in the page itself.
import("./appMain").catch((e: unknown) => {
  if (reloadForChunk()) return;
  const root = document.getElementById("root");
  if (root)
    root.textContent = `k7 could not load (${(e as Error)?.message ?? e}) — reload the page.`;
});
