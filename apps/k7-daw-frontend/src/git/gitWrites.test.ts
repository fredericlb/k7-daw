import { describe, expect, it, vi } from "vitest";
import {
  mergeGate,
  refusesFileWhileLocked,
  refusesWhileLocked,
} from "./gitLock";

vi.mock("../paraglide/messages.js", () => ({
  m: new Proxy({}, { get: (_t, k) => () => String(k) }),
}));

describe("the command gate", () => {
  it("refuses a writing verb and lets the ui and the play family through", () => {
    expect(refusesWhileLocked("track add gtr")).toBe(true);
    expect(refusesWhileLocked("transport record")).toBe(true);
    expect(refusesWhileLocked("selection delete")).toBe(true);
    expect(refusesWhileLocked("ui focus editor")).toBe(false);
    expect(refusesWhileLocked("transport play")).toBe(false);
  });
  it("says the merge waits, and refuses a file write", () => {
    expect(mergeGate({ kind: "file", method: "PUT", path: "/file" })).toBe(
      "git.lockedByMerge",
    );
    expect(mergeGate({ kind: "command", line: "track add x" })).toBe(
      "git.lockedByMerge",
    );
    expect(mergeGate({ kind: "command", line: "ui focus editor" })).toBe(null);
  });
});

// Every verb of the catalogue being classed is a Rust test (`verbs::ui::writes`).
describe("the verb table follows the registry", () => {
  it("lets a locked workspace keep every read and its own device's settings", () => {
    for (const line of [
      "sequence list",
      "library versions",
      "system channel list",
      "system plugins list vst",
      "project reload",
      "timeline play verse",
      "transport stop",
      "project console",
      "node engine vox-comp",
      "system channel type 3 stereo",
      "system cue 3+4",
      "system plugins scan",
    ])
      expect(refusesWhileLocked(line), line).toBe(false);
  });
  it("refuses what writes, a longer verb over its shorter one, and what it does not know", () => {
    for (const line of [
      "project console @k7-core/console/console-6",
      "node engine vox-comp vari-mu",
      "library upgrade k7",
      "transport metronome",
      "transport arm gtr",
      "track duplicate empty a",
      "no such verb",
    ])
      expect(refusesWhileLocked(line), line).toBe(true);
  });
});

describe("the file gate follows what the server refuses", () => {
  it("lets the reads and device routes through", () => {
    for (const [method, path] of [
      ["POST", "/preview"],
      ["POST", "/waveforms/prewarm"],
      ["POST", "/engine/project"],
      ["POST", "/engine/sync/retry"],
      ["PUT", "/preferences/vim"],
      ["POST", "/share/link"],
      ["POST", "/fmt"],
      ["POST", "/models/download"],
    ])
      expect(refusesFileWhileLocked(method, path), path).toBe(false);
  });
  it("refuses the routes the server's writes list names", () => {
    for (const [method, path] of [
      ["PUT", "/file?path=a"],
      ["POST", "/rename"],
      ["POST", "/entry/delete"],
      ["POST", "/upload"],
      ["POST", "/verb-host/render"],
    ])
      expect(refusesFileWhileLocked(method, path), path).toBe(true);
    expect(mergeGate({ kind: "file", method: "POST", path: "/preview" })).toBe(
      null,
    );
  });
});
