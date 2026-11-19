// srt.ts — the transcript as SRT text: a segment list written out and read back, in the SOURCE video's own seconds.
// Pure, so the verb counts cues in the browser's Worker while whisper itself runs on the Rust front's verb host.

/** One spoken segment, in the SOURCE video's own seconds. */
export interface TranscriptSegment {
  start: number;
  end: number;
  text: string;
}

// One cue's timing line — `00:01:02,480 --> 00:01:05,120`. whisper.cpp writes `.` where
// openai-whisper writes `,`, and both are read, because refusing a dialect is not ours to do.
const SRT_TIME =
  /^(\d{2,}):(\d{2}):(\d{2})[,.](\d{1,3})\s*-->\s*(\d{2,}):(\d{2}):(\d{2})[,.](\d{1,3})/;

const secondsOf = (h: string, m: string, s: string, ms: string): number =>
  Number(h) * 3600 +
  Number(m) * 60 +
  Number(s) +
  Number(ms.padEnd(3, "0")) / 1000;

/** The segments an SRT text holds, parsed defensively: a cue is its TIMING line plus what follows,
 *  so a missing or garbled index never shifts the text. Empty cues are dropped. */
export function parseSrt(text: string): TranscriptSegment[] {
  const out: TranscriptSegment[] = [];
  for (const block of text.replace(/\r\n?/g, "\n").split(/\n{2,}/)) {
    const lines = block.split("\n").filter((l) => l.trim() !== "");
    const at = lines.findIndex((l) => SRT_TIME.test(l));
    if (at < 0) continue;
    const hit = SRT_TIME.exec(lines[at]);
    if (!hit) continue;
    const content = lines
      .slice(at + 1)
      .join(" ")
      .trim();
    if (!content) continue;
    out.push({
      start: secondsOf(hit[1], hit[2], hit[3], hit[4]),
      end: secondsOf(hit[5], hit[6], hit[7], hit[8]),
      text: content,
    });
  }
  return out;
}

const srtTime = (t: number): string => {
  const ms = Math.max(0, Math.round(t * 1000));
  const two = (n: number) => String(n).padStart(2, "0");
  return (
    `${two(Math.floor(ms / 3600000))}:` +
    `${two(Math.floor((ms % 3600000) / 60000))}:` +
    `${two(Math.floor((ms % 60000) / 1000))},` +
    `${String(ms % 1000).padStart(3, "0")}`
  );
};

/** The content a transcript lane carries — valid SRT, so the text is one copy away from a subtitle file. */
export function srtOf(segments: TranscriptSegment[]): string {
  // Each block ends with its own newline, joined by one more: the blank line SRT puts between cues,
  // and a single trailing newline at the end of the file.
  return segments
    .map(
      (s, i) =>
        `${i + 1}\n${srtTime(s.start)} --> ${srtTime(s.end)}\n${s.text}\n`,
    )
    .join("\n");
}
