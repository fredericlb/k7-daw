#!/usr/bin/env bash
# device.sh — puts k7 on a USB-attached iPhone with a free Personal Team, and drives the suite and the log from the Mac.
# Usage: K7_IOS_TEAM=<team id> device.sh [all|build|install|forward|logs|devices] [--dry-run]  (docs: apps/k7-daw-engine/k7-runtime/docs/ios.md, "On a phone").
# What a reader gets wrong: the phone's engine runs on 18765 and its studio on 18799, never 8765, which is the user's own engine.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
bundle="${K7_IOS_BUNDLE:-rocks.k7.daw}"
studio_port="${K7_STUDIO_PORT:-18799}"
engine_port=18765
dd="${K7_IOS_DD:-$HOME/.cache/k7-ios-dd-device}"
out="${K7_IOS_OUT:-$HOME/.cache/k7-ios-device}"
app="$dd/Build/Products/Debug-iphoneos/k7.app"
step="all"
dry=0
for a in "$@"; do
  case "$a" in
    --dry-run) dry=1 ;;
    all | build | install | forward | logs | devices) step="$a" ;;
    *) echo "device: unknown argument '$a'" >&2; exit 64 ;;
  esac
done

say() { echo "device: $*"; }
run() {
  if [ "$dry" = 1 ]; then echo "  + $*"; else "$@"; fi
}

# The first real phone whose USB tunnel is up: a simulator also shows in devicectl's list, so it is filtered out.
phone_id() {
  xcrun devicectl list devices --json-output - --quiet 2>/dev/null | node -e '
    let s = "";
    process.stdin.on("data", (d) => (s += d)).on("end", () => {
      const ds = JSON.parse(s).result.devices.filter(
        (d) => d.hardwareProperties.reality !== "simulated" && d.hardwareProperties.platform === "iOS" && d.connectionProperties.tunnelState === "connected");
      if (ds[0]) console.log(process.argv[1] === "udid" ? ds[0].hardwareProperties.udid : ds[0].identifier);
    });' "${1:-}" || true
}

need_phone() {
  device="${K7_IOS_DEVICE:-$(phone_id)}"
  if [ -z "$device" ]; then
    if [ "$dry" = 1 ]; then
      say "no phone is attached over USB (a dry run goes on with a placeholder)"
      device="<phone>"
      return
    fi
    cat >&2 <<'MSG'
device: no iPhone is attached.
  - plug it in over USB, unlock it, and tap Trust on its screen
  - enable Settings > Privacy & Security > Developer Mode (restarts the phone) once
  - `xcrun devicectl list devices` must show it as connected (a simulator is not a phone)
  - K7_IOS_DEVICE=<id or name> names one when several are attached
MSG
    exit 3
  fi
}

team_help() {
  cat >&2 <<'MSG'
device: K7_IOS_TEAM is not set. It is your Personal Team's 10-character id:
  - Xcode > Settings > Accounts > select your Apple ID > Manage Certificates... (the team's id shows under "Personal Team"), or
  - security find-identity -v -p codesigning   (the id is the OU in the "Apple Development" certificate's subject:
    security find-certificate -c "Apple Development" -p | openssl x509 -noout -subject)
Sign in to Xcode with your Apple ID first (Settings > Accounts). A free team needs no paid subscription.
MSG
}

do_devices() {
  xcrun devicectl list devices
  id="$(phone_id)"
  if [ -n "$id" ]; then say "attached phone: $id"; else say "no phone is attached over USB"; fi
}

do_build() {
  if [ -z "${K7_IOS_TEAM:-}" ]; then
    if [ "$dry" = 1 ]; then team_help; say "(dry run goes on with a placeholder team)"; K7_IOS_TEAM="<TEAM>"; else team_help; exit 2; fi
  fi
  [ -d "$root/apps/k7-daw-ios/build/staged/frameworks/k7runtime.xcframework" ] || say "no staged frameworks: run node apps/k7-daw-ios/scripts/stage.mjs first (a device build, no --sim)"
  # Automatic signing, and no entitlements file: the project has none, and a Personal Team refuses push, iCloud and the memory entitlements.
  # The attached phone by its hardware UDID: a free team's profile names each device, and Xcode registers only the one it builds for.
  udid="$(phone_id udid)"
  if [ -n "$udid" ]; then dest="id=$udid"; else dest="generic/platform=iOS"; fi
  run xcodebuild -project "$root/apps/k7-daw-ios/k7.xcodeproj" -scheme k7 -configuration Debug \
    -destination "$dest" -derivedDataPath "$dd" \
    -allowProvisioningUpdates -allowProvisioningDeviceRegistration DEVELOPMENT_TEAM="$K7_IOS_TEAM" CODE_SIGN_STYLE=Automatic \
    PRODUCT_BUNDLE_IDENTIFIER="$bundle" build
}

do_install() {
  need_phone
  [ "$dry" = 1 ] || [ -d "$app" ] || { say "no app at $app: run '$0 build' first" >&2; exit 4; }
  run xcrun devicectl device install app --device "$device" "$app"
  # The engine's port is read once from engine-host.json in the app's container, which exists after the install.
  mkdir -p "$out"
  printf '{ "port": %s }\n' "$engine_port" > "$out/engine-host.json"
  run xcrun devicectl device copy to --device "$device" --domain-type appDataContainer --domain-identifier "$bundle" \
    --source "$out/engine-host.json" --destination "Library/Application Support/.k7/engine-host.json"
  # The studio's port goes in the launch environment; a free team's app shows "Untrusted Developer" until trusted in Settings.
  run xcrun devicectl device process launch --device "$device" --terminate-existing \
    --environment-variables "{\"K7_STUDIO_PORT\": \"$studio_port\"}" "$bundle"
}

do_forward() {
  if ! command -v iproxy >/dev/null 2>&1; then
    echo "device: iproxy is not installed. Install it yourself: brew install libimobiledevice (it also gives idevicesyslog)" >&2
    [ "$dry" = 1 ] || exit 5
  fi
  need_phone
  say "forwarding 127.0.0.1:$studio_port to the phone's $studio_port; stop with Ctrl-C"
  say "then, from the repo: E2E_IOS_ATTACH=1 npm run test:e2e:ios"
  # iproxy takes LOCAL:DEVICE and the hardware UDID, not the CoreDevice id; K7_IOS_UDID overrides the attached phone's.
  udid="${K7_IOS_UDID:-$(phone_id udid)}"
  run iproxy "$studio_port:$studio_port" ${udid:+-u "$udid"}
}

do_logs() {
  need_phone
  mkdir -p "$out"
  file="$out/system-$(date +%Y%m%d-%H%M%S).log"
  if command -v idevicesyslog >/dev/null 2>&1; then
    say "system log to $file (Ctrl-C ends it)"
    run sh -c "idevicesyslog ${K7_IOS_UDID:+-u $K7_IOS_UDID} --process k7 > '$file'"
  else
    say "idevicesyslog is not installed (brew install libimobiledevice); the app's own console goes to the file instead"
    run sh -c "xcrun devicectl device process launch --device '$device' --terminate-existing --console --environment-variables '{\"K7_STUDIO_PORT\": \"$studio_port\"}' '$bundle' > '$file' 2>&1"
  fi
  say "log: $file"
}

case "$step" in
  devices) do_devices ;;
  build) do_build ;;
  install) do_install ;;
  forward) do_forward ;;
  logs) do_logs ;;
  all)
    do_build
    do_install
    say "next, in two more terminals: '$0 forward' and '$0 logs'"
    ;;
esac
