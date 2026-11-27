// organizeStream.ts — the host's answer to an AI organize run, read: NDJSON lines until the `done` or the `error`.
// A line that is not JSON, or an event of a kind not known, is skipped; a stream that ends without either is an error.

import type { AiEvent, AiResult } from "@studio/k7-wasm/panel";

/** The panel the stream carries, its status lines passed on; throws the host's error, or that the stream stopped. */
export async function readOrganizeStream(
  body: ReadableStream<Uint8Array>,
  onStatus: (text: string) => void,
): Promise<AiResult> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buf = "";
  for (;;) {
    const { value, done } = await reader.read();
    buf += value ? decoder.decode(value, { stream: !done }) : "";
    const lines = buf.split("\n");
    buf = done ? "" : (lines.pop() ?? "");
    for (const line of lines) {
      const e = parse(line);
      if (!e) continue;
      if (e.type === "status") onStatus(e.text);
      else if (e.type === "done") return e.result;
      else if (e.type === "error") throw new Error(e.error);
    }
    if (done) throw new Error("the answer stopped before it was complete");
  }
}

/** A line as an event, or null for a blank line, text that is not JSON, or JSON that is no event. */
function parse(line: string): AiEvent | null {
  if (!line.trim()) return null;
  try {
    const e = JSON.parse(line) as AiEvent | null;
    return e && typeof e === "object" && typeof e.type === "string" ? e : null;
  } catch {
    return null;
  }
}
