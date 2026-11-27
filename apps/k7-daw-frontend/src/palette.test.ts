// The shell's own commands as the palette lists them: the registry is TypeScript, so its words are checked here; the merge and the ranking are Rust's.

import { describe, expect, it } from "vitest";
import {
  buildPaletteCommands,
  LOCAL_COMMANDS,
  LOCAL_READABLE,
} from "./palette";

describe("the shell's own commands in the palette", () => {
  it("are listed, each with a sentence", () => {
    const all = buildPaletteCommands([], "en");
    const local = all.find((c) => c.canonical === "ui help");
    expect(local?.readable).toContain("help");
    // Every shell command has a sentence: the palette cannot tell the two halves apart, so a missing one is an unfindable row.
    const missing = LOCAL_COMMANDS.filter((c) => !LOCAL_READABLE[c]);
    expect(missing).toEqual([]);
  });

  it("keep every description inside the palette's row, 60 chars", () => {
    // The same cap the crate's `palette.rs` holds commands.yaml to: the two halves are listed side by side, so a longer one truncates.
    const long = Object.entries(LOCAL_READABLE).flatMap(([name, r]) =>
      (["en", "fr"] as const)
        .filter((lang) => r[lang].length > 60)
        .map((lang) => `${name} (${lang}, ${r[lang].length}): ${r[lang]}`),
    );
    expect(long, "shell descriptions over 60 characters").toEqual([]);
  });
});
