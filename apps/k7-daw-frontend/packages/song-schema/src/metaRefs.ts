// metaRefs.ts — the `@meta/…` refs: the app's own built-ins (a lane, a note, an unsupported
// plugin's placeholder) and the curves an automation binding names. None of them has a
// folder, so a ref is all there is. Re-exported by moduleTypes.ts; see docs/moduleTypes.md.

import { META_NS, TRACK_REFS } from "@studio/k7-wasm/consts";

export {
  DOC_CATEGORY,
  DOC_REF,
  META_NS,
  NOT_SUPPORTED_CATEGORY,
  NOT_SUPPORTED_REF,
  PICTURE_STRIP,
  PICTURE_STRIP_SHADER,
  TRACK_REFS,
} from "@studio/k7-wasm/consts";

/** How a chain names a BUILT-IN — `@meta/track`, `@meta/IN` — the app's own vocabulary, never a scanned plugin (docs/moduleTypes.md). */
export const metaRef = (slug: string): string => `${META_NS}/${slug}`;

// --- curve refs (see apps/k7-daw-frontend/packages/song/docs/curves.md) -------------------------------

/** The type folder a curve ref names — the file still lives at `curves/<id>.k7x`, per-project; this is REF-CONSISTENCY only. */
const CURVES_CATEGORY = "curves";
const CURVE_REF_PREFIX = `${META_NS}/${CURVES_CATEGORY}/`;

/** How an automation binding names a curve: `@meta/curves/sweep` — docs/curves.md. */
export const curveRef = (id: string): string => `${CURVE_REF_PREFIX}${id}`;

/** The bare curve id an authored `curve:` field names, whichever form it was written in — docs/curves.md. */
export const curveIdFromRef = (ref: string): string =>
  ref.startsWith(CURVE_REF_PREFIX) ? ref.slice(CURVE_REF_PREFIX.length) : ref;

// --- the track modules (see docs/trackModules.md) --------------------------------

/** The modules a LANE is a node of — a sound lane is not typed by its signal, a picture lane is (docs/trackModules.md). */
export const [
  TRACK_REF,
  STRUDEL_TRACK_REF,
  GROUP_TRACK_REF,
  PICTURE_TRACK_REF,
] = TRACK_REFS;

/** The two output ports every non-group lane offers, from `@studio/graph-model` so the renderer cannot disagree (docs/moduleTypes.md). */
export { TRACK_OUTPUTS } from "@studio/graph-model";
