// What the palette LISTS, and in what order — the one part of "type what you want" that can be
// tested without a browser. The claim is that a person who knows what they want done finds the
// command without knowing what it is called.

import { describe, expect, it } from "vitest";
import {
  buildPaletteCommands,
  commandNameOf,
  LOCAL_COMMANDS,
  LOCAL_READABLE,
} from "./palette";
import type { CommandInfo } from "./types";

const cmd = (
  command: string,
  en: string,
  fr: string,
  args: CommandInfo["args"] = [],
): CommandInfo => ({ command, args, readable: { en, fr } });

const CATALOGUE: CommandInfo[] = [
  cmd(
    "track mute",
    "Mute or unmute a track",
    "Couper ou rétablir le son d'une piste",
    [{ name: "track", kind: "track", hint: "" }],
  ),
  cmd("transport play", "Start playback", "Lancer la lecture"),
  cmd(
    "project marker new",
    "Add a marker to the timeline",
    "Ajouter un repère sur la timeline",
  ),
  cmd("track color", "Recolour a track", "Recolorer une piste"),
];

const build = (locale = "en") => buildPaletteCommands(CATALOGUE, locale);

describe("the catalogue the palette lists", () => {
  it("carries the description in the reader's language", () => {
    expect(build("en")[0].readable).toBe("Mute or unmute a track");
    expect(build("fr")[0].readable).toBe(
      "Couper ou rétablir le son d'une piste",
    );
  });

  it("keeps the WHOLE table beside it, for the screen that names a command per language", () => {
    // The settings page places one field per locale and greys the catalogue's words into each — `readable` is one language by then.
    expect(build("en")[0].readableBy).toEqual({
      en: "Mute or unmute a track",
      fr: "Couper ou rétablir le son d'une piste",
    });
    // …and the same table whichever language built the list: it is the catalogue, not a view of it.
    expect(build("fr")[0].readableBy).toEqual(build("en")[0].readableBy);
  });

  it("falls back to the command's name rather than listing a blank row", () => {
    const bare = buildPaletteCommands(
      [{ command: "track solo", args: [] }],
      "en",
    );
    expect(bare[0].readable).toBe("track solo");
    // No table either: the line is not a translation, and a placeholder claiming to be one would offer English to the French field.
    expect(bare[0].readableBy).toBeUndefined();
  });

  it("adds the shell's own commands, described too", () => {
    const all = build();
    const local = all.find((c) => c.canonical === "ui help");
    expect(local?.readable).toContain("help");
    // Every shell command has a sentence: the palette cannot tell the two halves apart, so a missing one is an unfindable row.
    const missing = LOCAL_COMMANDS.filter((c) => !LOCAL_READABLE[c]);
    expect(missing).toEqual([]);
  });

  it("keeps every shell description inside the palette's row — 60 chars", () => {
    // The same cap the crate's `palette.rs` holds commands.yaml to: the two halves are listed side by side, so a longer one truncates.
    const long = Object.entries(LOCAL_READABLE).flatMap(([name, r]) =>
      (["en", "fr"] as const)
        .filter((lang) => r[lang].length > 60)
        .map((lang) => `${name} (${lang}, ${r[lang].length}): ${r[lang]}`),
    );
    expect(long, "shell descriptions over 60 characters").toEqual([]);
  });

  it("does not list a shell command the backend already offers", () => {
    const twice = buildPaletteCommands(
      [cmd("event move", "Move a clip", "Déplacer un clip")],
      "en",
    ).filter((c) => c.canonical === "event move");
    expect(twice).toHaveLength(1);
  });
});

describe("which command a line ran", () => {
  const all = build();

  it("takes the LONGEST name, as the dispatcher does", () => {
    expect(commandNameOf("track mute gtr", all)).toBe("track mute");
    expect(commandNameOf("track mute", all)).toBe("track mute");
  });

  it("answers null for a line that names no command", () => {
    expect(commandNameOf("wat is this", all)).toBe(null);
    // A name that merely STARTS the same is not a match — `track mutex` is not `track mute`.
    expect(commandNameOf("track mutex", all)).toBe(null);
  });
});
