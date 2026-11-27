// sceneFixture.ts — what a hand-built scene lacks and Rust fills in: a layer's schedule and the two built-in transitions.
// A reader would get wrong that this is test-only: no page builds a scene, the server's `scene_of` does (apps/k7-daw-wasm/verbs).

import { builtinTransitions, layerSchedule } from "@studio/k7-wasm/eyes";
import type {
  SceneClip,
  SceneTransition,
  VideoLayer,
  VideoScene,
} from "./scene";

/** A layer with the schedule the module computes for its clips. */
export const layerOf = (
  layer: Omit<VideoLayer, "schedule" | "clips" | "hidden"> &
    Partial<Pick<VideoLayer, "hidden">> & { clips: SceneClip[] },
): VideoLayer => ({
  hidden: false,
  ...layer,
  schedule: layerSchedule(layer.clips),
});

/** A scene with the built-in transitions, as `scene_of` carries them, and the song's own over them. */
export const sceneOf = (
  scene: Omit<VideoScene, "transitions">,
  own: Record<string, SceneTransition> = {},
): VideoScene => ({
  ...scene,
  transitions: { ...builtinTransitions(), ...own },
});
