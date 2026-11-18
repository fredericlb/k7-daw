// demuxProbe.ts — what an mp4, a .mov, a WebM or an mkv IS, read with mediabunny's demuxer where there is no ffprobe:
// the phone, and a browser's own recordings. It answers in ffprobe's names (codec, pix_fmt, container), so the plan
// cannot tell who read it. A WebCodecs codec string carries no chroma or depth for H.264 and HEVC: those come from the
// track's avcC/hvcC. Any other container is `undefined`, "cannot say". See docs/videoPrep.md.

import {
  FilePathSource,
  Input,
  type InputFormat,
  MATROSKA,
  MP4,
  QTFF,
  WEBM,
} from "mediabunny";
import type { VideoFacts } from "./probe";

/** ffprobe's format name for every member of the ISO family, .mov included. */
const ISO_CONTAINER = "mov,mp4,m4a,3gp,3g2,mj2";

/** ffprobe's format name for the Matroska family, WebM included. */
export const MATROSKA_CONTAINER = "matroska,webm";

const ISO: readonly InputFormat[] = [MP4, QTFF];

/** mediabunny's codec names, as ffprobe spells them. */
const CODECS: Record<string, string> = {
  avc: "h264",
  hevc: "hevc",
  vp8: "vp8",
  vp9: "vp9",
  av1: "av1",
  prores: "prores",
};

/** How many packets the frame rate is averaged over: enough to steady it, few enough to read at once. */
const RATE_PACKETS = 120;

/** The chroma each profile carries, for a codec string too short to state it; any other is 4:2:0. */
const CHROMA: Record<string, Record<string, string>> = {
  vp09: { "01": "444", "03": "444" },
  av01: { "1": "444", "2": "422" },
};

/** ffprobe's pix_fmt for a chroma ("420", "422", "444", "gray") at a bit depth. */
const pixFmt = (chroma: string, depth: number): string => {
  const base = chroma === "gray" ? "gray" : `yuv${chroma}p`;
  return depth > 8 ? `${base}${depth}le` : base;
};

/** The chroma a long codec string states: VP9's subsampling field, AV1's monochrome and subsampling fields. */
function statedChroma(kind: string, fields: string[]): string | undefined {
  if (kind === "vp09" && fields[4])
    return ["420", "420", "422", "444"][Number(fields[4])];
  if (kind === "av01" && fields[5]) {
    if (fields[4] === "1") return "gray";
    return { "11": "420", "10": "422", "00": "444" }[fields[5].slice(0, 2)];
  }
  return undefined;
}

/** ffprobe's pixel format from a WebCodecs codec string — `vp09.PP.LL.DD…`, `av01.P.LLT.DD…` — 8-bit 4:2:0 when it says nothing. */
export function pixFmtOf(param: string | null): string {
  const fields = (param ?? "").split(".");
  const [kind = "", profile = "", , depth] = fields;
  const stated = kind === "vp09" || kind === "av01";
  const bits = stated ? Number(depth) || 8 : 8;
  const chroma =
    (stated && (statedChroma(kind, fields) ?? CHROMA[kind]?.[profile ?? ""])) ||
    "420";
  return pixFmt(chroma, bits);
}

const CHROMA_IDC = ["gray", "420", "422", "444"];

/** ffprobe's pixel format from an avcC or hvcC body, the decoder config's description. */
export function configPixFmt(
  codec: "avc" | "hevc",
  config: Uint8Array,
): string {
  const b = config;
  if (codec === "hevc")
    return b.length > 17
      ? pixFmt(CHROMA_IDC[b[16] & 3] ?? "420", (b[17] & 7) + 8)
      : "";
  if (b.length < 2) return pixFmt("420", 8);
  const profile = b[1];
  if (![100, 110, 122, 144, 244].includes(profile)) return pixFmt("420", 8);
  const u16 = (i: number) => (b[i] << 8) | b[i + 1];
  // The High profiles state chroma and depth after the parameter sets; reached by skipping them.
  let i = 5;
  const sps = b[i++] & 0x1f;
  for (let n = 0; n < sps && i + 2 <= b.length; n++) i += 2 + u16(i);
  const pps = b[i++] ?? 0;
  for (let n = 0; n < pps && i + 2 <= b.length; n++) i += 2 + u16(i);
  if (i + 2 <= b.length)
    return pixFmt(CHROMA_IDC[b[i] & 3] ?? "420", (b[i + 1] & 7) + 8);
  if (profile === 110) return pixFmt("420", 10);
  if (profile === 122) return pixFmt("422", 10);
  if (profile === 144 || profile === 244) return pixFmt("444", 10);
  return pixFmt("420", 8);
}

/** ProRes states nothing in a config: 4444 is 4:4:4, the rest 4:2:2, 10-bit as ffprobe reports them. */
const proresPixFmt = (fourcc: string | null): string =>
  fourcc === "ap4h" || fourcc === "ap4x" ? "yuv444p10le" : "yuv422p10le";

/** The facts of `source`'s first video stream; null with none; undefined when it is neither ISO nor Matroska. */
export async function probeWithoutFfmpeg(
  source: string,
): Promise<Omit<VideoFacts, "noFfmpeg"> | null | undefined> {
  const input = new Input({
    source: new FilePathSource(source),
    formats: [...ISO, WEBM, MATROSKA],
  });
  try {
    let format: InputFormat;
    try {
      format = await input.getFormat();
    } catch {
      return undefined;
    }
    const iso = ISO.includes(format);
    const track = await input.getPrimaryVideoTrack();
    if (!track) return null;
    const codec = await track.getCodec();
    const param = await track.getCodecParameterString();
    // An ISO sample entry mediabunny does not know has no codec string, so no pixel format is claimed for it.
    let pix = codec || !iso ? pixFmtOf(param) : "";
    if (codec === "prores") pix = proresPixFmt(param);
    else if (codec === "avc" || codec === "hevc") {
      const d = (await track.getDecoderConfig())?.description;
      const bytes = !d
        ? undefined
        : ArrayBuffer.isView(d)
          ? new Uint8Array(d.buffer, d.byteOffset, d.byteLength)
          : new Uint8Array(d);
      if (bytes) pix = configPixFmt(codec, bytes);
      else if (iso) pix = "";
    }
    // An ISO file's rotation is left unread on purpose: docs/videoPrep.md, "A rotated stream gets an upright copy".
    const rotation = iso ? 0 : await track.getRotation();
    return {
      codec: codec ? (CODECS[codec] ?? codec) : "",
      width: await track.getCodedWidth(),
      height: await track.getCodedHeight(),
      pixFmt: pix,
      fps: (await track.computePacketStats(RATE_PACKETS)).averagePacketRate,
      seconds: await input.computeDuration(),
      container: iso ? ISO_CONTAINER : MATROSKA_CONTAINER,
      ...(rotation ? { rotation } : {}),
    };
  } finally {
    input.dispose();
  }
}
