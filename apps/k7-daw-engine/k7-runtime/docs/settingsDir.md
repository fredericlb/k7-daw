# The settings folder

The engine reads `--settings-dir <folder>`, else `$K7_SETTINGS_DIR`, once at startup, before anything opens
the properties. The flag wins, and it is removed from the arguments so the positional port and mode parsing
never see it. Unset, nothing changes: the user's own engine keeps JUCE's folder and `~/.k7`.

| choice | why |
|---|---|
| `Settings.xml` goes in the folder, through a `PropertyStorage` whose `getAppPrefsFolder()` names it (`engine/SettingsStorage.h`), and every engine the binary builds (server, `--render`, the dumps, the multi-edit probe) is made with it | on macOS JUCE asks the system for the home folder, so `HOME` does not move the file: a test's engine writes the user's real `~/Library/Studio/Settings.xml` |
| the plugin-scan cache `exts.json` goes in the same folder when one is set | the scan writes the user's real `~/.k7/exts.json` whenever `STUDIO_HOME` was not also set |
| the supervisor's `extsCache()` follows `$K7_SETTINGS_DIR` | its rescan deletes the file the engine writes |
| nothing else under `<state>` moves (the guards, instruments, record cache) | those stay under `$STUDIO_HOME`, which every harness already sets; moving them would split one state folder in two |
| an env value is read by the engine and by its children (the supervisor's engine, the scan child) | a harness sets it once on the process it starts |
| `[startup] settings-dir=<folder>` is logged when one is set | a run that wrote somewhere unexpected can be traced from its log |

A reader of `<state>/exts.json` (the TypeScript and the Rust listings) looks under `$STUDIO_HOME`, so a run that
sets `K7_SETTINGS_DIR` and then lists `@engine/` modules finds no cache; set it to the folder `STUDIO_HOME` names
when the listing must see the scan.

A harness that runs the real engine passes `K7_SETTINGS_DIR` (a scratch folder under the run's own home), as
`apps/k7-daw-engine/scripts/ci/e2e-*.sh` and `e2e/src/engineRender.ts` do. `scripts/settings-dir-proof.mjs`
(`npm run test:settings-dir --workspace k7-runtime`) renders and scans with a scratch folder and checks that the
user's `Settings.xml` and `exts.json` keep their modification times; the choice of folder is covered by
`tests/SettingsDir_test.cpp`.
