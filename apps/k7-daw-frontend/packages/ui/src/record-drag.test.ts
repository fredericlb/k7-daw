// The drag a set list's row carries; the rules on a record are the Rust crate's, tested in song-edit's records.rs.

import { describe, expect, it } from "vitest";
import { packRecordDrag, parseRecordDrag } from "./record-drag";

describe("the drag a row carries", () => {
  it("round-trips, and refuses anything else", () => {
    expect(parseRecordDrag(packRecordDrag({ record: "a/b.wav" }))).toEqual({
      record: "a/b.wav",
    });
    expect(parseRecordDrag('{"song":"a"}')).toBeNull();
    expect(parseRecordDrag("not json")).toBeNull();
  });
});
