// scene.ts — the renderer's input, generated from Rust (`apps/k7-daw-wasm/eyes`): a viewport, an fps and a stack of layers in decimal seconds.
// A reader would get wrong that eyes knows nothing else: it imports none of the brain's packages, so it runs in a browser.

import type { SceneClip, ScenePass } from "@studio/k7-wasm/types";

export type {
  LayerSchedule,
  SceneCaptions,
  SceneClip,
  SceneFade,
  SceneKey,
  ScenePass,
  SceneShape,
  SceneTransition,
  ScheduleRamp,
  ScheduleSpan,
  VideoLayer,
  VideoScene,
} from "@studio/k7-wasm/types";

/** A shot's animated properties, each its sorted keys in seconds from the shot's start. */
export type SceneAnimate = NonNullable<SceneClip["animate"]>;

/** Is this pass, at its knobs as they are NOW, one that changes nothing — skipped per frame, see docs/passes.md. */
export const isIdentityPass = (p: ScenePass): boolean =>
  !!p.identity &&
  Object.entries(p.identity).every(([k, v]) => (p.params[k] ?? 0) === v);
