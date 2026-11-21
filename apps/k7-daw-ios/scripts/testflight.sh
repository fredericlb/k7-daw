#!/usr/bin/env bash
# testflight.sh — stages a device build, archives it in Release, and uploads it to App Store Connect for TestFlight.
# Usage: [K7_IOS_TEAM=<team> K7_ASC_KEY_ID=<key> K7_ASC_ISSUER=<issuer>] testflight.sh [--no-stage] [--export-only] [--allow-dirty] [--dry-run]
# Docs: apps/k7-daw-engine/k7-runtime/docs/ios.md, "TestFlight".
# What a reader gets wrong: the build number is the commit count, so a second upload from the same commit is refused.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
ios="$root/apps/k7-daw-ios"
out="${K7_IOS_ARCHIVES:-$HOME/.cache/k7-ios-testflight}"
stage=1
dest="upload"
dirty_ok=0
dry=0
for a in "$@"; do
  case "$a" in
    --no-stage) stage=0 ;;
    --export-only) dest="export" ;;
    --allow-dirty) dirty_ok=1 ;;
    --dry-run) dry=1 ;;
    *) echo "testflight: unknown argument '$a'" >&2; exit 64 ;;
  esac
done

say() { echo "testflight: $*"; }

# The ids kept out of the public tree: K7_TESTFLIGHT_ENV (default ~/.config/k7/testflight.env), KEY=VALUE lines; the shell's own values win.
env_file="${K7_TESTFLIGHT_ENV:-$HOME/.config/k7/testflight.env}"
if [ -f "$env_file" ]; then
  while IFS='=' read -r key value; do
    case "$key" in '' | \#*) continue ;; esac
    [ -n "${!key:-}" ] || export "$key=$value"
  done < "$env_file"
  say "ids read from $env_file"
fi
run() {
  if [ "$dry" = 1 ]; then echo "  + $*"; else "$@"; fi
}
fail() {
  if [ "$dry" = 1 ]; then say "$1 (a dry run goes on)"; else echo "testflight: $1" >&2; exit "$2"; fi
}

[ -n "${K7_IOS_TEAM:-}" ] || fail "K7_IOS_TEAM is not set: your paid team's 10-character id (developer.apple.com > Membership details)" 2
K7_IOS_TEAM="${K7_IOS_TEAM:-TEAMID}"

# The App Store Connect API key signs in for both the provisioning and the upload; only its path is passed, never its content.
key_id="${K7_ASC_KEY_ID:-}"
issuer="${K7_ASC_ISSUER:-}"
key_path="${K7_ASC_KEY_PATH:-$HOME/.appstoreconnect/private_keys/AuthKey_${key_id}.p8}"
auth=()
if [ -n "$key_id" ] && [ -n "$issuer" ]; then
  [ -f "$key_path" ] || fail "no API key file at $key_path (download it from App Store Connect > Users and Access > Integrations)" 3
  auth=(-authenticationKeyPath "$key_path" -authenticationKeyID "$key_id" -authenticationKeyIssuerID "$issuer")
elif [ "$dest" = "upload" ]; then
  fail "K7_ASC_KEY_ID and K7_ASC_ISSUER are not set: the upload needs an App Store Connect API key (--export-only builds the .ipa without one)" 3
fi

# A build from uncommitted changes would carry a number that names a commit it is not.
if [ "$dirty_ok" = 0 ] && [ -n "$(git -C "$root" status --porcelain --untracked-files=no)" ]; then
  fail "the working tree has uncommitted changes: commit them, or pass --allow-dirty" 4
fi

build="$(git -C "$root" rev-list --count HEAD)"
archive="$out/k7-$build.xcarchive"
export_dir="$out/k7-$build"
say "build number $build ($(git -C "$root" rev-parse --short HEAD)), destination: $dest"
[ -n "${K7_GITHUB_CLIENT_ID:-}" ] || say "K7_GITHUB_CLIENT_ID is not set: this build offers no Sign in with GitHub"

if [ "$stage" = 1 ]; then
  run node "$ios/scripts/stage.mjs"
fi

mkdir -p "$out"
run rm -rf "$archive" "$export_dir"
run xcodebuild -project "$ios/k7.xcodeproj" -scheme k7 -configuration Release \
  -destination "generic/platform=iOS" -archivePath "$archive" \
  -allowProvisioningUpdates ${auth[@]+"${auth[@]}"} \
  DEVELOPMENT_TEAM="$K7_IOS_TEAM" CODE_SIGN_STYLE=Automatic archive

opts="$out/ExportOptions.plist"
cat > "$opts" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>method</key><string>app-store-connect</string>
  <key>destination</key><string>$dest</string>
  <key>teamID</key><string>$K7_IOS_TEAM</string>
  <key>signingStyle</key><string>automatic</string>
  <key>uploadSymbols</key><true/>
  <key>manageAppVersionAndBuildNumber</key><false/>
  <key>testFlightInternalTestingOnly</key><true/>
</dict>
</plist>
PLIST

run xcodebuild -exportArchive -archivePath "$archive" -exportPath "$export_dir" \
  -exportOptionsPlist "$opts" -allowProvisioningUpdates ${auth[@]+"${auth[@]}"}

if [ "$dry" = 1 ]; then
  say "dry run: nothing was built or uploaded"
elif [ "$dest" = "upload" ]; then
  say "uploaded build $build: it shows in TestFlight once App Store Connect has processed it (usually 5 to 15 minutes)"
else
  say "the .ipa is in $export_dir; nothing was uploaded"
fi
