// wav.ts — the RIFF/WAVE chunk walk and header every WAV reader and writer here shares, with no decode.
// What reads wrong: `dataBytes` trusts the disk over a 0 or oversized `data` size, `declaredBytes` is the header's word (docs/analysis.md).

export interface WavLayout {
  /** The `fmt ` chunk's body, copied as it is into a header of its own. */
  format: Buffer;
  dataOffset: number;
  /** The samples there are: a 0 or oversized `data` size reads as what is on disk, in whole frames. */
  dataBytes: number;
  /** The `data` size exactly as the header states it. */
  declaredBytes: number;
  blockAlign: number;
  sampleRate: number;
}

const PCM = 1;
const FLOAT = 3;
const EXTENSIBLE = 0xfffe;

/** Whether a `fmt ` body describes uncompressed samples, the only kind measured or cut by bytes. */
function isPlain(fmt: Buffer): boolean {
  const tag = fmt.readUInt16LE(0);
  if (tag === PCM || tag === FLOAT) return true;
  // The sub-format GUID starts with the plain format tag it stands for.
  if (tag === EXTENSIBLE && fmt.length >= 26) {
    const sub = fmt.readUInt16LE(24);
    return sub === PCM || sub === FLOAT;
  }
  return false;
}

/** Where a PCM or float WAV's samples are, from its head and its size on disk; null for anything else. */
export function wavLayout(head: Buffer, fileSize: number): WavLayout | null {
  if (
    head.length < 12 ||
    head.toString("ascii", 0, 4) !== "RIFF" ||
    head.toString("ascii", 8, 12) !== "WAVE"
  )
    return null;
  let at = 12;
  let format: Buffer | null = null;
  while (at + 8 <= head.length) {
    const id = head.toString("ascii", at, at + 4);
    const size = head.readUInt32LE(at + 4);
    const body = at + 8;
    if (id === "fmt ") {
      if (size < 16 || body + size > head.length) return null;
      format = Buffer.from(head.subarray(body, body + size));
    } else if (id === "data") {
      if (!format || !isPlain(format)) return null;
      const blockAlign = format.readUInt16LE(12);
      const sampleRate = format.readUInt32LE(4);
      if (!blockAlign || !sampleRate) return null;
      const there = Math.max(0, fileSize - body);
      // A recorder that never closed its file leaves the size at 0 or past the end: what is there is the data.
      const dataBytes =
        size === 0 || size > there ? there - (there % blockAlign) : size;
      return {
        format,
        dataOffset: body,
        dataBytes,
        declaredBytes: size,
        blockAlign,
        sampleRate,
      };
    }
    // Chunks are word-aligned; an odd size carries a pad byte the walk has to step over.
    at = body + size + (size & 1);
  }
  return null;
}

/** The header of a WAV holding `dataBytes` of samples in `format`. */
export function wavHeader(format: Buffer, dataBytes: number): Buffer {
  const pad = format.length & 1;
  const h = Buffer.alloc(12 + 8 + format.length + pad + 8);
  h.write("RIFF", 0, "ascii");
  h.writeUInt32LE(h.length - 8 + dataBytes, 4);
  h.write("WAVE", 8, "ascii");
  h.write("fmt ", 12, "ascii");
  h.writeUInt32LE(format.length, 16);
  format.copy(h, 20);
  const data = 20 + format.length + pad;
  h.write("data", data, "ascii");
  h.writeUInt32LE(dataBytes, data + 4);
  return h;
}

/** The 16-byte `fmt ` body of interleaved 32-bit float samples. */
export function floatFormat(channels: number, sampleRate: number): Buffer {
  const f = Buffer.alloc(16);
  f.writeUInt16LE(FLOAT, 0);
  f.writeUInt16LE(channels, 2);
  f.writeUInt32LE(sampleRate, 4);
  f.writeUInt32LE(sampleRate * channels * 4, 8);
  f.writeUInt16LE(channels * 4, 12);
  f.writeUInt16LE(32, 14);
  return f;
}
