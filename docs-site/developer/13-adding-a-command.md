# Adding a command

A new action should be a command, so that it exists at once in the command bar, the help page, the right-click menus, the Commands settings page (where a person can give it a key), and on a second device. Buttons then dispatch the line rather than calling a function of their own.

First decide **where it runs**:

| It… | Goes |
|---|---|
| changes project or workspace files, or asks the engine through the file service | the catalogue: `apps/k7-daw-frontend/packages/commands` |
| only changes what the shell shows (a page, a panel, a tool, a zoom) | a `ui` command in the frontend |
| needs something only the browser has (the clipboard, Web MIDI, a camera, a download) but writes through a file route | a bare-named web-local command |

## A catalogue verb

1. **Declare it** in `apps/k7-daw-wasm/verbs/commands.yaml`:

   ```yaml
   - command: "track colour"
     sid: track.colour
     readable:
       en: "Set a track's colour"
       fr: "Choisir la couleur d'une piste"
     help: "set a track's colour (e.g. track colour gtr #3fa34d)"
     surfaces: [cli]
     args:
       [
         { name: track, complete: tracks },
         { name: colour, hint: "a #rrggbb colour" },
       ]
   ```

   Pick the first word for the entity the command acts on: the settings page groups by it, and the right-click menu matches on the **leading arguments** (here a `tracks` value, so it appears on a track's menu). Use `menu:` only to narrow that. Add `confirm: true` if the menu should ask first. Give every free-text argument a `hint`.

2. **Write the handler** in a family file under `apps/k7-daw-wasm/verbs/src/verbs/` and add its row to the SID table in
   `apps/k7-daw-wasm/verbs/src/verbs/mod.rs`, then write `commands.json` again with
   `node apps/k7-daw-wasm/verbs/scripts/catalogue.mjs`:

   ```rust
   pub fn track_colour(a: &str, h: &mut Host) -> Done {
       let w = split_words(a);
       if !h.project()?.tracks.iter().any(|t| t.id == w[0]) {
           h.refuse([format!("no track \"{}\"", w[0])]);
           return Ok(());
       }
       // …write through song-edit, which changes the value and nothing else
       h.log([format!("{} is {}", w[0], w[1])]);
       h.reload();
       Ok(())
   }
   ```

   Refuse with a sentence that says what to do instead. Write files through the song-edit writers so an edit stays a
   one-line diff. A verb that needs the machine calls a route of the front (`h.call`), never a program of its own. The
   writes land as one undo step on their own.

3. **Test it** in the family's test file over `apps/k7-daw-wasm/verbs/tests/support/mod.rs` (`cargo test` in the crate).

4. **Use it** from the UI by dispatching the line: `void runCommand(\`track colour ${id} ${hex}\`)`. A knob-like edit that
   must not reload the shell uses `quietCommand`.

Nothing else is needed: the palette and the help page read the catalogue, the crate completes its arguments from the
provider, and the menu derives itself from the catalogue.

### A new completion provider

Add it to the crate's providers in `apps/k7-daw-wasm/verbs/src/complete/mod.rs`, then give it a class and a hint in the table of `apps/k7-daw-wasm/verbs/src/complete/provider_info.rs` (a test fails when a name is in one and not the other). The provider and class names are mirrored on the wire as the `ArgProvider` and `ArgClass` unions in `apps/k7-daw-frontend/packages/web-protocol/commandTypes.ts`; add the new name there too.

## A `ui` command

1. Add an entry to the group file it belongs to under `apps/k7-daw-frontend/src/ui/commands/`: its name, its sentence in both languages (60 characters), its arguments, and `run`, returning `true`. `UI_EXACT`, `UI_HEADS`, `LOCAL_COMMANDS`, `LOCAL_READABLE` and `LOCAL_ARGS` are derived from that registry. A fixed vocabulary goes in `makeListArg` (`apps/k7-daw-frontend/src/palette.ts`).
2. If the help page should describe it, add it to `VIEW_COMMANDS` in `apps/k7-daw-frontend/src/components/HelpPage.tsx`.
3. If a one-surface device should answer it, add it to `ExternalSurface`'s `runLocal` (`apps/k7-daw-frontend/src/components/ExternalSurface.tsx`); otherwise it answers that it is editor-only.

`localCommands.test.ts` fails if the palette lists a line the dispatcher does not recognise.

## A key for it

Add a binding to `KEYS` in `apps/k7-daw-frontend/src/keys.ts`. A plain string is a default a person can move; a conditional binding (`{ if: "caret_track", command: "…{caret_track}" }`, or a list tried in order) stays the app's. Conditions are computed in `App.tsx`'s `conditions` memo. Add a case to `keys.test.ts`. See [Keys, palette and menus](./14-keys-palette-menus.md).
