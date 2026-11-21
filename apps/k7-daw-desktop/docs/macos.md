# K7 DAW on a Mac

K7 DAW is the Mac's k7: Apple silicon, macOS 13 or later.

| choice | why |
|---|---|
| Apple silicon only, macOS 13 as the floor | AudioWorklet, WKDownload and the media-capture delegate are there; an Intel Mac was never served |
| the same bundle identifier as the other desktop builds | one identifier names the data folder on every OS, and a Developer ID signature does not care about the prefix |
| a regular app (Dock icon, app menu, Cmd-Q) | `docs/desktop.md` |
| targets `app` and `dmg` | the DMG is the download |

## Signing

Tauri signs the main binary and the `.app`; the staged resources under `Contents/Resources` hold programs of
their own, and notarization refuses any unsigned Mach-O. `scripts/macSign.mjs` signs them first, inside-out (libraries
before programs), at the end of `npm run stage`, with the hardened runtime.

| binary | entitlements | why |
|---|---|---|
| the app | JIT, unsigned executable memory, audio input, camera | Wasmtime compiles machine code in the front and maps it executable with mprotect, not MAP_JIT, which the hardened runtime kills without the unsigned-memory key; the page asks for the camera, and the engine it launches records through the app's microphone grant |
| node | JIT, unsigned executable memory, no library validation | V8 and the native addons the backend loads |
| the engine (`k7-runtime`) | audio input, unsigned executable memory, no library validation | it records, and hosts plugins (AU, VST3) signed by others |
| the supervisor and the ONNX dylib | the hardened runtime alone | they do nothing that needs more |

No sandbox: the app runs the engine, which opens any audio device and any plugin folder.

**The microphone** belongs to the responsible process, the app: the engine records through it, so `Info.plist`
carries `NSMicrophoneUsageDescription` and `NSCameraUsageDescription`, and the app's entitlements carry the two
device keys. Without the plist key macOS hands the engine silence.

## Without a Developer ID

The identity is `$APPLE_SIGNING_IDENTITY`, else `-`: an ad-hoc signature, which Apple silicon requires at the least.
The DMG then runs on the Mac that built it; any other Mac's Gatekeeper refuses it. An ad-hoc build is a new identity
at every build, so the microphone and camera grants are asked again after each rebuild.

Stage and `desktop:build` take the same identity, or notarization refuses the mix:

```
npm run stage --workspace k7-desktop
npm run desktop:build --workspace k7-desktop
```

A DMG's window layout is set through Finder with AppleScript, which times out on a busy or headless Mac; `CI=true`
in the environment skips it, and the DMG is then a plain one.

## With a Developer ID

`APPLE_SIGNING_IDENTITY` names the "Developer ID Application" certificate; `APPLE_API_ISSUER`, `APPLE_API_KEY` and
`APPLE_API_KEY_PATH` are an App Store Connect API key, with which Tauri notarizes the `.app`. `scripts/macNotarize.mjs <dmg>`
then signs the DMG, submits it and staples the ticket: Gatekeeper refuses a download that was not notarized.
