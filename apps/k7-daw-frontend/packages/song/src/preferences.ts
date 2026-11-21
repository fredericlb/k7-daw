// preferences.ts — the user's GLOBAL preferences (`<state>/preferences.k7x`): machine and rig config that
// spans every project, unlike the per-project song files. The default audio interface the engine opens, plus
// human-readable labels for its channels — labels are for reading only, unrelated to track names.
// Every read and write of the file goes through here; an old YAML is upgraded by the Rust server alone: docs/preferencesFile.md.

import { join } from "node:path";
import { stub } from "./editStub";
import { studioHome } from "./studioHome";

/** Through `studioHome`, never hand-joined — engine-client/docs/paths.md, and the doc that names this file. */
export const PREFS_PATH = join(studioHome(), "preferences.k7x");
export type ChannelDir = "in" | "out";
export interface ChannelLabel {
  ch: number[]; // one or more 1-based channel numbers ([3, 4] == a stereo pair)
  label: string;
}
// The chosen interface. `type` is the driver backend, and they are NOT interchangeable — ASIO is low-latency.
export interface AudioDevicePref {
  name: string;
  type?: string;
}
// Editor settings live here, not the browser: they belong to the person. `vim` defaults OFF: a touch keyboard has no hardware keys for it.
export interface EditorPref {
  vim: boolean;
}
export interface Preferences {
  audioDevice?: AudioDevicePref; // preferred audio interface (engine opens it on connect)
  channels: { in: ChannelLabel[]; out: ChannelLabel[] };
  /** Which inputs are STEREO, as each pair's 1-based FIRST channel — see apps/k7-daw-engine/k7-runtime/docs/audioDevice.md. */
  inputPairs: number[];
  editor: EditorPref;
  /** The chain that SEPARATES a record into stems — one answer for the whole machine, asked once. */
  stemsChain?: string;
  /** …and the one that gives the record WITHOUT ITS VOICE, asked the same way — see apps/k7-daw-engine/k7-supervisor/docs/stemSplit.md. */
  instrumentalChain?: string;
  /** Which output pair the `CUE` bus leaves by — a label from `channels.out`, or the numbers. See docs/cue.md. */
  cueOutput?: string | number[];
}

// The defaults, in ONE place — every other module builds on this rather than its own literal.
export const defaultPreferences = (): Preferences => ({
  channels: { in: [], out: [] },
  inputPairs: [],
  editor: { vim: false },
});
// Coerce a parse into a well-formed Preferences; exported so a test need not read the real state dir.
export function normalizePreferences(v: any): Preferences {
  return stub("song/preferences.normalizePreferences", v);
}

/** The k7x text's preferences; a text that does not read is the defaults. */
export function parsePreferences(text: string): Preferences {
  return stub("song/preferences.parsePreferences", text);
}

/** The text written to `preferences.k7x`: normalized, so no read ever meets a stray field. */
export function stringifyPreferences(p: Preferences): string {
  return stub("song/preferences.stringifyPreferences", p);
}

/** The preferences as found, and the reason when a file is there that does not read (the prefs are then the defaults). */
export interface Found {
  prefs: Preferences;
  problem?: string;
}

/** The preferences, or the defaults when no file reads: a project opens either way. */
export function loadPreferences(): Preferences {
  return stub("song/preferences.loadPreferences");
}

/** Why the preferences file cannot be changed, or null when it can; mutators throw this. */
export function preferencesProblem(): string | null {
  return stub("song/preferences.preferencesProblem");
}

export function savePreferences(p: Preferences): void {
  stub("song/preferences.savePreferences", p);
}

// --- mutators (load, change, save; return the new prefs) ----------------------

export function setAudioDevice(name: string, type?: string): Preferences {
  return stub("song/preferences.setAudioDevice", name, type);
}

/** Remember whether the pair at `channel` is stereo — either half is accepted, the odd one stored. */
export function setInputPair(channel: number, stereo: boolean): Preferences {
  return stub("song/preferences.setInputPair", channel, stereo);
}

/** Remember the headphone pair. An empty spec clears it — "this machine has nowhere to send a cue". */
export function setCueOutput(spec: string | number[] | undefined): Preferences {
  return stub("song/preferences.setCueOutput", spec);
}

/** Remember which chain separates. An empty ref clears it — "this machine separates nothing". */
export function setStemsChain(ref: string): Preferences {
  return stub("song/preferences.setStemsChain", ref);
}

/** Remember which chain gives the instrumental. An empty ref clears it, like `setStemsChain`. */
export function setInstrumentalChain(ref: string): Preferences {
  return stub("song/preferences.setInstrumentalChain", ref);
}

export function setEditorVim(on: boolean): Preferences {
  return stub("song/preferences.setEditorVim", on);
}

// "Focusrite USB (ASIO)", or "(system default)" when nothing is configured.
export function formatAudioDevice(d: AudioDevicePref | undefined): string {
  return stub("song/preferences.formatAudioDevice", d);
}

export function setChannelLabel(
  dir: ChannelDir,
  ch: number[],
  label: string,
): Preferences {
  return stub("song/preferences.setChannelLabel", dir, ch, label);
}

export function clearChannelLabel(dir: ChannelDir, ch: number[]): Preferences {
  return stub("song/preferences.clearChannelLabel", dir, ch);
}

// --- helpers -----------------------------------------------------------------

// Parse a channel spec like "3+4" or "1" into channel numbers.
export function parseChannels(spec: string): number[] {
  return stub("song/preferences.parseChannels", spec);
}

// "3+4 synth" for a group, "1 guitar" for a single channel.
export function formatChannel(e: ChannelLabel): string {
  return stub("song/preferences.formatChannel", e);
}

// A one-line summary per direction, e.g. { in: "1 guitar · 3+4 synth", out: "1+2 monitors" }.
export function formatChannels(p: Preferences): { in: string; out: string } {
  return stub("song/preferences.formatChannels", p);
}

// A track's `input` -> physical channels; an unresolvable LABEL reports itself rather than monitoring wrongly.
export function resolveInput(
  input: string | number[] | undefined,
  p: Preferences,
): { channels: number[]; unknownLabel?: string } {
  return stub("song/preferences.resolveInput", input, p);
}

/** The `CUE` bus's pair -> physical channels, against the OUT labels. Empty means this machine has no headphone pair. */
export function resolveCueOutput(p: Preferences): {
  channels: number[];
  unknownLabel?: string;
} {
  return stub("song/preferences.resolveCueOutput", p);
}
