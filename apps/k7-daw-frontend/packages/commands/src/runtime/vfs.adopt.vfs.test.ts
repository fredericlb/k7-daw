// vfs.adopt.vfs.test.ts — a job's files in the Vfs: an adopt is posted where the file ends, never where it was only passed through.

import { beforeEach, describe, expect, it } from "vitest";
import { Vfs } from "./vfs.ts";

let vfs: Vfs;

const adopts = () => vfs.staged().filter((o) => o.kind === "adopt");
const info = { len: 4, seconds: 1.5 };

beforeEach(() => {
  vfs = new Vfs(true);
  vfs.mkdir("/w/p.k7", true);
});

describe("vfs.adopt", () => {
  it("holds the file as a binary of the job's length and posts one adopt", () => {
    vfs.adopt("j1", "mix.wav", "/w/p.k7/mix.wav", info);
    expect(vfs.files.get("/w/p.k7/mix.wav")).toMatchObject({
      len: 4,
      binary: true,
      seconds: 1.5,
    });
    expect(vfs.secondsOf("/w/p.k7/mix.wav")).toBe(1.5);
    expect(vfs.staged()).toEqual([
      { kind: "adopt", job: "j1", out: "mix.wav", file: "/w/p.k7/mix.wav" },
    ]);
    expect(() => vfs.readText("/w/p.k7/mix.wav")).toThrow(/EROFS/);
  });

  it("asks to replace a file that was there", () => {
    vfs.load("/w/p.k7/old.wav", { len: 9, binary: true });
    vfs.adopt("j1", "mix.wav", "/w/p.k7/old.wav", info);
    expect(adopts()).toEqual([
      expect.objectContaining({ file: "/w/p.k7/old.wav", replace: true }),
    ]);
  });

  it("makes the folders it needs and refuses a folder", () => {
    vfs.adopt("j1", "a.wav", "/w/p.k7/new/deep/a.wav", info);
    expect(vfs.stat("/w/p.k7/new/deep")?.kind).toBe(1);
    vfs.mkdir("/w/p.k7/dir", false);
    expect(() => vfs.adopt("j1", "b.wav", "/w/p.k7/dir", info)).toThrow(
      /EISDIR/,
    );
  });

  // The copy of an adopted file is another adopt: a `copy` would name a file the server does not have yet.
  it("posts an adopt, never a copy, for the copy of an adopted file", () => {
    vfs.adopt("j1", "mix.wav", "/w/p.k7/a.wav", info);
    vfs.copy("/w/p.k7/a.wav", "/w/p.k7/b.wav");
    expect(vfs.staged().map((o) => o.kind)).toEqual(["adopt", "adopt"]);
    expect(vfs.staged().some((o) => o.kind === "copy")).toBe(false);
    expect(vfs.files.get("/w/p.k7/b.wav")?.seconds).toBe(1.5);
  });

  it("posts one adopt at the new name for a move", () => {
    vfs.adopt("j1", "mix.wav", "/w/p.k7/a.wav", info);
    vfs.rename("/w/p.k7/a.wav", "/w/p.k7/b.wav");
    expect(vfs.staged()).toEqual([
      { kind: "adopt", job: "j1", out: "mix.wav", file: "/w/p.k7/b.wav" },
    ]);
  });

  // `renderToWav` renders into a temporary folder and copies the mix out.
  it("does not post an adopt left under /tmp, and posts the copy taken out of it", () => {
    vfs.mkdir("/tmp/r1", true);
    vfs.adopt("j1", "mix.wav", "/tmp/r1/mix.wav", info);
    expect(vfs.staged()).toEqual([]);
    vfs.copy("/tmp/r1/mix.wav", "/w/p.k7/mix.wav");
    expect(vfs.staged()).toEqual([
      { kind: "adopt", job: "j1", out: "mix.wav", file: "/w/p.k7/mix.wav" },
    ]);
  });

  it("does not post an adopt that was removed", () => {
    vfs.adopt("j1", "mix.wav", "/w/p.k7/a.wav", info);
    vfs.remove("/w/p.k7/a.wav", false);
    expect(vfs.staged()).toEqual([]);
  });

  it("forgets the adopt when the run is rolled back", () => {
    vfs.adopt("j1", "mix.wav", "/w/p.k7/a.wav", info);
    vfs.rollback();
    expect(vfs.stat("/w/p.k7/a.wav")).toBeUndefined();
    expect(vfs.staged()).toEqual([]);
  });

  it("is the server's file once settled: a later copy is a plain copy", () => {
    vfs.adopt("j1", "mix.wav", "/w/p.k7/a.wav", info);
    vfs.settle();
    expect(vfs.staged()).toEqual([]);
    vfs.copy("/w/p.k7/a.wav", "/w/p.k7/c.wav");
    expect(vfs.staged()).toEqual([
      { kind: "copy", from: "/w/p.k7/a.wav", file: "/w/p.k7/c.wav" },
    ]);
  });
});
