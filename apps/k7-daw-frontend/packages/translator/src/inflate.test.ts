// An imported archive may not inflate past a ceiling: a few kilobytes of zeros declare gigabytes.

import { gzipSync, zipSync } from "fflate";
import { describe, expect, it } from "vitest";
import {
  boundedGunzip,
  boundedUnzip,
  MAX_ENTRIES,
  MAX_INFLATED_BYTES,
} from "./inflate";

const MiB = 2 ** 20;
const zeros = (n: number) => new Uint8Array(n);

describe("boundedUnzip", () => {
  it("refuses one entry past the ceiling", () => {
    const zip = zipSync({ "project.xml": zeros(2 * MiB) });
    expect(zip.length).toBeLessThan(MiB);
    expect(() => boundedUnzip(zip, MiB)).toThrow(/inflates past 1 MiB/);
  });

  it("refuses entries that pass alone and not together", () => {
    const zip = zipSync({ a: zeros(0.6 * MiB), b: zeros(0.6 * MiB) });
    expect(() => boundedUnzip(zip, MiB)).toThrow(/refused/);
  });

  it("reads an archive under it", () => {
    const zip = zipSync({ a: new Uint8Array([1, 2, 3]) });
    expect(boundedUnzip(zip).a).toEqual(new Uint8Array([1, 2, 3]));
  });

  it("caps at half a gibibyte by default: what a browser's memory holds", () => {
    expect(MAX_INFLATED_BYTES).toBe(512 * MiB);
  });

  // A zip bomb of empty files declares no bytes at all and still costs a table entry each.
  it("refuses an archive naming more files than a project holds", () => {
    const names: Record<string, Uint8Array> = {};
    for (let i = 0; i <= MAX_ENTRIES; i++) names[`f${i}`] = new Uint8Array(0);
    expect(() => boundedUnzip(zipSync(names))).toThrow(/more than/);
  });
});

describe("boundedGunzip", () => {
  it("refuses a stream whose trailer declares more than the ceiling", () => {
    expect(() => boundedGunzip(gzipSync(zeros(2 * MiB)), MiB)).toThrow(
      /inflates past/,
    );
  });

  // The trailer is the file's own claim: a stream that lies low must not grow past it either.
  it("never inflates past a trailer that lies low: the output is cut at what it said", () => {
    const real = gzipSync(zeros(4 * MiB));
    const lie = real.slice();
    new DataView(lie.buffer).setUint32(lie.length - 4, 16, true);
    expect(boundedGunzip(lie, MiB).length).toBeLessThanOrEqual(16);
  });

  it("inflates one under it", () => {
    expect(boundedGunzip(gzipSync(zeros(10))).length).toBe(10);
  });
});
