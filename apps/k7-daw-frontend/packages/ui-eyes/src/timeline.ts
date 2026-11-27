// timeline.ts — which clips a layer shows at time t, where in their sources, and the transition running between them.
// A reader would get wrong that the rule is not here: Rust works it out into `layer.schedule`, and this only looks the span up.
// No WebGL, no DOM, no clock, so realtime calls it per rAF tick and export per frame. See song-video/docs/transitions.md.

import type {
  SceneClip,
  ScheduleRamp,
  ScheduleSpan,
  VideoLayer,
} from "./scene";

export interface Active {
  clip: SceneClip;
  // Where in the SOURCE file this maps to, in seconds: sourceFrom + (t - start) * speed.
  sourceTime: number;
  // Where this placement stops; Infinity for an open-ended last one.
  end: number;
  // Its position in `layer.clips` — the driver's element key, since a clip's id is its REF and repeats.
  index: number;
}

/** A shot inside its fade's STRETCH this frame: which run, and how present the shot is — 1 whole, 0 gone. */
export interface SlotStretch {
  chain: string;
  presence: number;
}

/** What a layer draws at `t`: up to two pictures, the program between them when a fade runs, and each one's stretch. */
export interface LayerFrame {
  from: Active | null; // the picture going OUT — null means the transparent texel
  to: Active | null; // …and the one coming IN
  // null == draw `to` plain, as a cut does
  blend: {
    name: string;
    progress: number;
    stretch?: { from?: SlotStretch; to?: SlotStretch };
  } | null;
}

const clamp01 = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);

const ramp = (r: ScheduleRamp, t: number): number => {
  const v = clamp01((t - r.start) / r.len);
  return r.rising ? v : 1 - v;
};

// The span covering `t`: the last one that has begun, if it has not ended.
function spanAt(layer: VideoLayer, t: number): ScheduleSpan | undefined {
  const spans = layer.schedule.spans;
  let lo = 0;
  let hi = spans.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (spans[mid].at <= t) lo = mid + 1;
    else hi = mid;
  }
  const s = spans[lo - 1];
  return s && (s.until === undefined || t < s.until) ? s : undefined;
}

const activeOf = (layer: VideoLayer, i: number, t: number): Active => {
  const clip = layer.clips[i];
  return {
    clip,
    // The one mapping speed has: signed, so a freeze holds `sourceFrom` and a reverse runs back from it.
    sourceTime: clip.sourceFrom + (t - clip.start) * (clip.speed ?? 1),
    end: layer.schedule.ends[i] ?? Number.POSITIVE_INFINITY,
    index: i,
  };
};

/** The whole frame for one layer — which two pictures, how far between them, and which run through a stretch. */
export function layerFrameAt(layer: VideoLayer, t: number): LayerFrame {
  const s = spanAt(layer, t);
  if (!s) return { from: null, to: null, blend: null };
  const b = s.blend;
  return {
    from: s.from === undefined ? null : activeOf(layer, s.from, t),
    to: s.to === undefined ? null : activeOf(layer, s.to, t),
    blend: b
      ? {
          name: b.name,
          progress: ramp(b.progress, t),
          ...(b.stretchFrom || b.stretchTo
            ? {
                stretch: {
                  ...(b.stretchFrom
                    ? {
                        from: {
                          chain: b.stretchFrom.chain,
                          presence: ramp(b.stretchFrom.presence, t),
                        },
                      }
                    : {}),
                  ...(b.stretchTo
                    ? {
                        to: {
                          chain: b.stretchTo.chain,
                          presence: ramp(b.stretchTo.presence, t),
                        },
                      }
                    : {}),
                },
              }
            : {}),
        }
      : null,
  };
}

/** The shot a picture SLOT holds this frame — 0 the outgoing side of a blend or the only picture of a cut, 1 the incoming. */
export function slotActive(frame: LayerFrame, slot: 0 | 1): Active | null {
  if (slot === 1) return frame.blend ? frame.to : null;
  return frame.blend ? frame.from : frame.to;
}

/** Each slot's shot's EFFECT this frame, by `slotActive`'s slots. */
export function slotEffects(
  frame: LayerFrame,
  out: [string | null, string | null] = [null, null],
): [string | null, string | null] {
  out[0] = slotActive(frame, 0)?.clip.effect ?? null;
  out[1] = slotActive(frame, 1)?.clip.effect ?? null;
  return out;
}

/** The clip on TOP at `t`, or null — the last in file order, which is the one covering the rest. */
export function activeAt(layer: VideoLayer, t: number): Active | null {
  const s = spanAt(layer, t);
  return s ? activeOf(layer, s.top, t) : null;
}

/** Is a layer of SHAPES on at `t`? Its clips are spans saying the lane is on, and one stating none is on throughout. */
export function shapesVisibleAt(layer: VideoLayer, t: number): boolean {
  return !layer.clips.length || !!spanAt(layer, t);
}
