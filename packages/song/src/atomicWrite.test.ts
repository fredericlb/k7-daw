// atomicWrite.test.ts — a write lands whole, a failed one leaves the old file and no temp behind.

import fs, {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  atomicTempFor,
  createFileExclusive,
  isAtomicTemp,
  writeFileAtomic,
} from "./atomicWrite";

const dir = () => mkdtempSync(join(tmpdir(), "atomic-"));

describe("writeFileAtomic", () => {
  it("replaces a file's content and leaves nothing beside it", () => {
    const d = dir();
    const f = join(d, "mix.k7x");
    writeFileSync(f, "old\n");
    writeFileAtomic(f, "new\n");
    expect(readFileSync(f, "utf8")).toBe("new\n");
    expect(readdirSync(d)).toEqual(["mix.k7x"]);
  });

  it("writes bytes and creates a missing file", () => {
    const d = dir();
    const f = join(d, "a.bin");
    writeFileAtomic(f, new Uint8Array([1, 2, 3]));
    expect([...readFileSync(f)]).toEqual([1, 2, 3]);
  });

  it("leaves the old file whole and removes its temp when the write fails", () => {
    const d = dir();
    const f = join(d, "mix.k7x");
    writeFileSync(f, "old\n");
    // Not a string nor bytes: the write throws after the temp is open.
    expect(() =>
      writeFileAtomic(f, { length: 4 } as unknown as Uint8Array),
    ).toThrow();
    expect(readFileSync(f, "utf8")).toBe("old\n");
    expect(readdirSync(d)).toEqual(["mix.k7x"]);
  });

  it("leaves the target alone and removes its temp when the rename fails", () => {
    const d = dir();
    const f = join(d, "taken");
    mkdirSync(f);
    writeFileSync(join(f, "inside"), "x");
    expect(() => writeFileAtomic(f, "new")).toThrow();
    expect(readdirSync(d)).toEqual(["taken"]);
    expect(readFileSync(join(f, "inside"), "utf8")).toBe("x");
  });

  it.skipIf(process.platform === "win32")(
    "keeps an existing file's mode",
    () => {
      const d = dir();
      const f = join(d, "run.sh");
      writeFileSync(f, "#!/bin/sh\n");
      chmodSync(f, 0o751);
      writeFileAtomic(f, "#!/bin/sh\necho\n");
      expect(statSync(f).mode & 0o777).toBe(0o751);
    },
  );

  it("names a hidden temp in the same folder, and recognises it", () => {
    const t = atomicTempFor("/w/song.k7/nodes/mix.k7x");
    expect(dirname(t)).toBe(join("/w", "song.k7", "nodes"));
    expect(basename(t).startsWith(".mix.k7x.")).toBe(true);
    expect(isAtomicTemp(basename(t))).toBe(true);
    expect(isAtomicTemp("mix.k7x")).toBe(false);
    expect(isAtomicTemp("notes.tmp")).toBe(false);
  });
});

describe("createFileExclusive", () => {
  it("makes a missing file whole, with the mode given, and leaves no temp", () => {
    const d = dir();
    const f = join(d, "first.k7x");
    expect(createFileExclusive(f, "one\n", { mode: 0o600 })).toBe(true);
    expect(readFileSync(f, "utf8")).toBe("one\n");
    if (process.platform !== "win32")
      expect(statSync(f).mode & 0o777).toBe(0o600);
    expect(readdirSync(d)).toEqual(["first.k7x"]);
  });

  it("refuses an existing file, which keeps its bytes, and leaves no temp", () => {
    const d = dir();
    const f = join(d, "first.k7x");
    writeFileSync(f, "mine\n");
    expect(createFileExclusive(f, "theirs\n")).toBe(false);
    expect(readFileSync(f, "utf8")).toBe("mine\n");
    expect(readdirSync(d)).toEqual(["first.k7x"]);
  });
});

describe.each(["EPERM", "ENOTSUP", "ENOSYS"])(
  "createFileExclusive where link gives %s",
  (code) => {
    const realLink = fs.linkSync;
    afterEach(() => {
      (fs as { linkSync: unknown }).linkSync = realLink;
      syncBuiltinESMExports();
    });
    const noLinks = () => {
      (fs as { linkSync: unknown }).linkSync = () => {
        throw Object.assign(new Error(code), { code });
      };
      syncBuiltinESMExports();
    };

    it("with a lock held publishes by rename, whole, and refuses a taken name", () => {
      noLinks();
      const d = dir();
      const f = join(d, "first.k7x");
      expect(createFileExclusive(f, "one\n", { heldLock: true })).toBe(true);
      expect(readFileSync(f, "utf8")).toBe("one\n");
      expect(createFileExclusive(f, "two\n", { heldLock: true })).toBe(false);
      expect(readFileSync(f, "utf8")).toBe("one\n");
      expect(readdirSync(d)).toEqual(["first.k7x"]);
    });

    it("without one falls back to an exclusive open, which also refuses a taken name", () => {
      noLinks();
      const d = dir();
      const f = join(d, "held.lock");
      expect(createFileExclusive(f, "token\n")).toBe(true);
      expect(createFileExclusive(f, "other\n")).toBe(false);
      expect(readFileSync(f, "utf8")).toBe("token\n");
    });

    it.skipIf(process.platform === "win32")(
      "counts a link to nothing as a taken name",
      () => {
        noLinks();
        const d = dir();
        const f = join(d, "first.k7x");
        fs.symlinkSync(join(d, "nowhere"), f);
        expect(createFileExclusive(f, "one\n", { heldLock: true })).toBe(false);
        expect(fs.lstatSync(f).isSymbolicLink()).toBe(true);
      },
    );
  },
);

describe("a rename a scanner holds on Windows", () => {
  const realRename = fs.renameSync;
  const platform = Object.getOwnPropertyDescriptor(
    process,
    "platform",
  ) as PropertyDescriptor;
  afterEach(() => {
    (fs as { renameSync: unknown }).renameSync = realRename;
    syncBuiltinESMExports();
    Object.defineProperty(process, "platform", platform);
    vi.restoreAllMocks();
  });

  it("still lands on the third try where Atomics.wait throws", () => {
    vi.spyOn(Atomics, "wait").mockImplementation(() => {
      throw new Error("cannot block in this thread");
    });
    Object.defineProperty(process, "platform", { value: "win32" });
    let calls = 0;
    (fs as { renameSync: unknown }).renameSync = (from: string, to: string) => {
      if (++calls <= 2)
        throw Object.assign(new Error("EPERM"), { code: "EPERM" });
      return realRename(from, to);
    };
    syncBuiltinESMExports();
    const d = dir();
    const f = join(d, "mix.k7x");
    writeFileSync(f, "old\n");
    writeFileAtomic(f, "new\n");
    expect(calls).toBe(3);
    expect(readFileSync(f, "utf8")).toBe("new\n");
    expect(readdirSync(d)).toEqual(["mix.k7x"]);
  });
});
