// aiTurnStream.ts — the host's answer to one turn, read: NDJSON lines until the `turn` or the `error`. A line
// that is not JSON, or an event of a kind not known, is skipped; a stream that ends without either is an error.

import type { AiTurnReply } from "./aiLoop";
import type { AiTurnEvent } from "./aiOrganize";

/** The turn the stream carries, its status lines passed on; throws the host's error, or that the stream stopped. */
export async function readTurnStream(
  body: ReadableStream<Uint8Array>,
  onStatus: (text: string) => void,
): Promise<AiTurnReply> {
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
      else if (e.type === "turn") return { text: e.text, content: e.content };
      else if (e.type === "error") throw new Error(e.error);
    }
    if (done) throw new Error("the answer stopped before it was complete");
  }
}

/** A line as an event, or null for a blank line, text that is not JSON, or JSON that is no event. */
function parse(line: string): AiTurnEvent | null {
  if (!line.trim()) return null;
  try {
    const e = JSON.parse(line) as Record<string, unknown> | null;
    switch (e?.type) {
      case "status":
        return typeof e.text === "string"
          ? { type: "status", text: e.text }
          : null;
      case "turn":
        return typeof e.text === "string"
          ? { type: "turn", text: e.text, content: e.content }
          : null;
      case "error":
        return typeof e.error === "string"
          ? { type: "error", error: e.error }
          : null;
      case "ping":
        return { type: "ping" };
      default:
        return null;
    }
  } catch {
    return null;
  }
}
