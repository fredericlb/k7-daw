// The resolvers of a typed name: what is scored is Rust's (`fuzzyScores`), what is picked among the scores is the browser's collator.
// The rules of a path itself are song-edit's and are pinned there.

import { beforeAll, describe, expect, it } from "vitest";
import { wasmReady } from "./boot.ts";
import {
  resolveEditorFile as resolve,
  resolveEntry,
  resolveModuleFile,
  resolveSequence,
} from "./paths.ts";

beforeAll(async () => {
  await wasmReady();
});

const TREE = [
  "clips",
  "clips/kick.wav",
  "clips/rythm.k7x",
  "modules",
  "modules/instruments/synth",
  "modules/instruments/synth/module.k7x",
  "modules/audio/ampli",
  "modules/audio/ampli/module.k7x",
  "sequences/melody.k7x",
];

describe("resolveEntry", () => {
  it("takes an exact path as it stands", () => {
    expect(resolveEntry("clips/kick.wav", TREE)).toBe("clips/kick.wav");
  });

  it("is fuzzy, like every other name the shell resolves", () => {
    // The same subsequence rule the file picker uses: `rythm` finds `clips/rythm.k7x`, which a prefix filter could not.
    expect(resolveEntry("rythm", TREE)).toBe("clips/rythm.k7x");
  });

  it("answers a FOLDER too — `fs file move` and `ui pin` are offered on one", () => {
    expect(resolveEntry("clips", TREE)).toBe("clips");
  });

  it("answers null rather than a wrong guess", () => {
    expect(resolveEntry("zzzz", TREE)).toBeNull();
    expect(resolveEntry("", TREE)).toBeNull();
  });
});

describe("resolveModuleFile", () => {
  it("prefers a module over anything else the name could mean", () => {
    // `ui builder ampli` names a MODULE, so a clip that matches as well must not win.
    expect(resolveModuleFile("ampli", TREE)).toBe(
      "modules/audio/ampli/module.k7x",
    );
    expect(resolveModuleFile("synth", TREE)).toBe(
      "modules/instruments/synth/module.k7x",
    );
  });

  it("falls back to the whole project, so a full path can be pasted in", () => {
    expect(resolveModuleFile("sequences/melody.k7x", TREE)).toBe(
      "sequences/melody.k7x",
    );
  });
});

const OPEN_FILES = [
  "01.danse.k7/clips/audio_14_0.k7x",
  "01.danse.k7/clips/audio_14_1.k7x",
  "01.danse.k7/clips/vox_0.k7x",
  "01.danse.k7/nodes/gtr.k7x",
  "01.danse.k7/sequences/riff.k7x",
  "01.danse.k7/project.k7x",
];

describe("what ui file resolves", () => {
  it("takes an exact path as it stands", () => {
    expect(resolve("01.danse.k7/clips/vox_0.k7x", OPEN_FILES)).toBe(
      "01.danse.k7/clips/vox_0.k7x",
    );
  });

  it("takes a bare id — which is what `ui file <track>` is for", () => {
    expect(resolve("gtr", OPEN_FILES)).toBe("01.danse.k7/nodes/gtr.k7x");
  });

  it("resolves a PLACEMENT id to its material's file", () => {
    // The reported bug, verbatim. `@98.5` appears in no path, so the name alone finds nothing.
    expect(resolve("audio_14_0@98.5", OPEN_FILES)).toBe(
      "01.danse.k7/clips/audio_14_0.k7x",
    );
  });

  it("keeps the two placements of one material on the same file", () => {
    // They are two events and one definition — which is the whole reason a placement id exists.
    expect(resolve("audio_14_0@0", OPEN_FILES)).toBe(
      resolve("audio_14_0@98.5", OPEN_FILES),
    );
  });

  it("lets the written name win over the material", () => {
    // Second and not first: a name that matches something is the answer, whatever an `@` in it might also have meant.
    expect(resolve("audio_14_1", OPEN_FILES)).toBe(
      "01.danse.k7/clips/audio_14_1.k7x",
    );
  });

  it("answers null when nothing matches, rather than the text that was typed", () => {
    // Falling back to `arg` would open a path nobody named and report an error about it over an already-popped pane.
    expect(resolve("zzz-nothing-like-this@4", OPEN_FILES)).toBeNull();
  });

  it("answers null on an empty project rather than inventing a path", () => {
    expect(resolve("audio_14_0@98.5", [])).toBeNull();
  });
});

const SEQ_FILES = [
  "sequences/intro.k7x",
  "sequences/ge.rythm.k7x",
  "clips/intro.k7x",
  "tracks/intro.k7x",
  "modules/audio/verb/module.k7x",
];

describe("resolveSequence", () => {
  it("prefers a SEQUENCE over a clip or a track of the same name", () => {
    // The whole reason this is not the editor's generic resolver: `intro` is three files here, and only one has notes to draw.
    expect(resolveSequence("intro", SEQ_FILES)).toBe("sequences/intro.k7x");
  });

  it("takes an exact path as itself", () => {
    expect(resolveSequence("clips/intro.k7x", SEQ_FILES)).toBe(
      "clips/intro.k7x",
    );
  });

  it("still falls back to the rest of the project, so a path can be pasted", () => {
    expect(resolveSequence("verb/module", SEQ_FILES)).toBe(
      "modules/audio/verb/module.k7x",
    );
  });

  it("matches a dotted sequence id fuzzily", () => {
    expect(resolveSequence("ge.rythm", SEQ_FILES)).toBe(
      "sequences/ge.rythm.k7x",
    );
  });

  it("says nothing rather than opening an empty grid", () => {
    expect(resolveSequence("zzz nothing", SEQ_FILES)).toBeNull();
    expect(resolveSequence("", SEQ_FILES)).toBeNull();
  });
});
