# iCloud on the Mac

A Mac app given a provisioning profile shares with the person's iPhone and iPad what the iOS app keeps in iCloud: the
workspace list and the Forgejo servers (the key-value store, `apps/k7-daw-backend/docs/roam.md`), and the GitHub and
Forgejo sign-ins and the one SSH key (iCloud Keychain, `apps/k7-daw-backend/docs/git.md`, "Credentials"). A build without
a profile is the plain one, with every sign-in, list and key kept on the Mac. The code is `src-tauri/src/icloud.rs` and `scripts/desktopBuild.mjs`.

## Choices

| choice | why |
|---|---|
| the entitlements are added only when `K7_MAC_PROVISIONING_PROFILE` names a profile, which is embedded as `Contents/embedded.provisionprofile`; otherwise the build prints one line that iCloud sync is off | the key-value store and keychain groups are restricted entitlements: macOS refuses to launch an app that carries them with no profile allowing them |
| the build reads the App ID prefix and the team from the profile and writes literal values (`<prefix>.rocks.k7.daw`) into a copy of `macos/app.entitlements` | codesign expands no `$(TeamIdentifierPrefix)`, which Xcode alone substitutes; a profile of another App ID or without iCloud is refused before the build |
| the entitlements are `com.apple.application-identifier`, `com.apple.developer.team-identifier`, `keychain-access-groups` (`<prefix>.rocks.k7.daw` first) and `com.apple.developer.ubiquity-kvstore-identifier` (`<team>.rocks.k7.daw`) | the same group and store identifier as the iOS app's `k7.entitlements`, so both read one keychain group and one store |
| a profile needs a real `APPLE_SIGNING_IDENTITY`, one the profile lists | an ad-hoc signature matches no profile |
| the app turns iCloud on when its bundle holds `embedded.provisionprofile`, before the front starts: the front's roaming store and the synced keychain for the tokens and the SSH key | the profile and the entitlements come together from the build, so one file says both |
| the key-value store is reached through the Objective-C runtime's C functions (`objc_msgSend` cast per message), with no binding crate | a few messages of one class do not earn a dependency; the casts are the price |
| the store passes the front's keys alone (`ws.`, `forge.`), and is read on every call, never cached | iCloud updates the store in place; a read is current |
| the Mac signs in with the Apple devices' one shared SSH key: it adopts the iPhone's or iPad's when iCloud holds one, and otherwise puts its own there | the user's choice, one key to manage for every Apple device (`apps/k7-daw-backend/docs/git.md`, "Credentials") |
| the Mac's previous key stays on the Mac as its old key, tried once when a host refuses the shared one, until the person forgets it under the key in the Connect dialog | its hosts list that key: a host not yet given the shared key keeps working, and the page names it |
| a profile is per machine for development (the Mac's provisioning UDID is in it) and per team for Developer ID | Apple's rule for each kind of profile |

## How to build with iCloud

On developer.apple.com, Certificates, Identifiers & Profiles, with the team S5TM69LZLJ:

1. **Identifiers**: open the App ID `rocks.k7.daw` (the iOS app's). Its platforms must include macOS. Tick **iCloud**
   (key-value storage needs no container) and save. Keychain sharing needs no switch: every profile allows the team's groups.
2. **Devices** (development only): register this Mac as a macOS device with its Provisioning UDID, from
   `system_profiler SPHardwareDataType | grep "Provisioning UDID"`.
3. **Certificates**: an **Apple Development** certificate for this Mac (Xcode → Settings → Accounts → Manage
   Certificates makes one), or **Developer ID Application** for a build others download.
4. **Profiles** → **+**: **macOS App Development** (or **Developer ID** under Distribution), App ID `rocks.k7.daw`,
   the certificate of step 3, this Mac (development), then download the `.provisionprofile`.
5. Build, with the certificate's full name as the identity for both steps:

```
export APPLE_SIGNING_IDENTITY="Apple Development: <name> (<id>)"
export K7_MAC_PROVISIONING_PROFILE=~/Downloads/<profile>.provisionprofile
npm run stage --workspace k7-desktop
npm run desktop:build --workspace k7-desktop
```

The merged log's `[host]` lines say at start whether iCloud sync is on. A profile made after a change to the App ID
(step 1) is a new download: an older one does not carry the new entitlement.
