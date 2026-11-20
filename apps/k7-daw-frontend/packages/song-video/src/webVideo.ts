// webVideo.ts — a compiled picture as the browser reads it, every media path made relative by the caller's own `toRel`.
// Pure: the file service answers `/video`, and a verb in the Worker builds the very scene a server render takes (docs/serverRender.md).

import { basename } from "node:path";
import type { WebVideoProject, WebVideoTrack } from "@studio/web-protocol";
import type { VideoProjectDoc, VideoTrackDoc } from "./videoDoc";
import { videoDuration } from "./videoTracks";

/** The picture of a song that has none — never a throw, since every song may carry one. */
export function emptyVideoProject(dir: string): WebVideoProject {
  return {
    name: basename(dir),
    viewport: [0, 0],
    fps: 0,
    duration: 0,
    markers: [],
    videoTracks: [],
    framePlugins: [],
    sound: null,
    transitions: {},
    stretches: {},
  };
}

/** A compiled picture as the browser reads it, every path workspace-relative. */
export function webVideoProjectOf(
  p: VideoProjectDoc,
  toRel: (abs: string) => string,
): WebVideoProject {
  return {
    name: p.name,
    ...(p.rowOrder ? { rowOrder: [...p.rowOrder] } : {}),
    viewport: p.viewport,
    fps: p.fps,
    duration: videoDuration(p.videoTracks),
    markers: p.markers.map((m) => ({ sec: m.sec, name: m.name, tc: m.tc })),
    videoTracks: toWebVideoTracks(p.videoTracks, toRel),
    framePlugins: p.framePlugins.map((pl) => webPlugin(pl, toRel)),
    sound: p.sound,
    transitions: Object.fromEntries(
      Object.entries(p.transitions).map(([k, t]) => [
        k,
        { ...t, params: { ...t.params } },
      ]),
    ),
    stretches: Object.fromEntries(
      Object.entries(p.stretches).map(([k, run]) => [
        k,
        run.map((pl) => webPlugin(pl, toRel)),
      ]),
    ),
    ...(p.timelines
      ? {
          timelines: Object.fromEntries(
            Object.entries(p.timelines).map(([k, lanes]) => [
              k,
              toWebVideoTracks(lanes, toRel),
            ]),
          ),
        }
      : {}),
  };
}

/** A song's picture lanes as the browser reads them, every media path made workspace-relative. */
export function toWebVideoTracks(
  videoTracks: VideoTrackDoc[],
  toRel: (abs: string) => string,
): WebVideoTrack[] {
  return videoTracks.map((t) => ({
    id: t.id,
    name: t.name,
    color: t.color,
    mute: t.mute,
    solo: t.solo,
    // The LENS wired to this lane, when one is, and when it opens — packages/song/docs/camera.md.
    ...(t.camera === undefined
      ? {}
      : { camera: t.camera, cameraMonitor: t.cameraMonitor ?? "on" }),
    x: t.x,
    y: t.y,
    w: t.w,
    h: t.h,
    // Workspace-RELATIVE on the way out: the browser fetches every byte back through /media.
    clips: t.clips.map((c) => ({
      id: c.id,
      name: c.name,
      src: c.source ? toRel(c.source) : "",
      ...(c.optimized ? { optimized: toRel(c.optimized) } : {}),
      ...(c.proxy ? { proxy: toRel(c.proxy) } : {}),
      ...(c.still ? { still: true as const } : {}),
      start: c.start,
      length: c.length,
      sourceFrom: c.sourceFrom,
      // A fade names its kind; the kind's GLSL rides on the project, once per name.
      ...(c.fadeIn ? { fadeIn: { ...c.fadeIn } } : {}),
      ...(c.fadeOut ? { fadeOut: { ...c.fadeOut } } : {}),
      ...(c.animate ? { animate: c.animate } : {}),
      ...(c.speed !== undefined ? { speed: c.speed } : {}),
      ...(c.effect ? { effect: c.effect } : {}),
      ...(c.captions ? { captions: c.captions } : {}),
      ...(c.timeline ? { timeline: c.timeline } : {}),
    })),
    // Shapes carry no media path — each variant's fields, `id` included, pass straight through.
    shapes: t.shapes.map((s) => ({ ...s })),
    // …and this lane's own video run — a shader's GLSL travels INLINE, so there is no path here.
    ...(t.plugins?.length
      ? { plugins: t.plugins.map((p) => webPlugin(p, toRel)) }
      : {}),
  }));
}

/** A picture plugin for the browser: its LUT made workspace-relative, as every path the browser fetches back is. */
export function webPlugin<P extends { lut?: string }>(
  p: P,
  toRel: (abs: string) => string,
): P {
  return { ...p, ...(p.lut ? { lut: toRel(p.lut) } : {}) };
}
