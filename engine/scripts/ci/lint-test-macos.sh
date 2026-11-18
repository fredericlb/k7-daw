#!/bin/bash
# lint-test-macos.sh — OneDev's "lint and test macos": what the container's "lint and test" runs, natively on the macOS agent.
# A ci/ branch's TypeScript gate; main keeps the container's, Linux being the image's platform (docs/release.md).
set -euo pipefail
# The agent's own Node, newer than .nvmrc's: the firewall lets it take LAN connections, which WebRTC's ICE needs.
node --version
npm ci --no-audit --no-fund
# Each Rust workspace's lock still pins its manifests (docs/lints.md).
npm run lint:locks
# The engine testbed, in a target folder of the agent's own and through sccache when it has one (docs/rust-builds.md).
while IFS= read -r kv; do export "${kv?}"; done < <(node engine/scripts/sccache.mjs checks)
target="$HOME/k7-trees/testbed-target"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=4
(cd apps/k7/backend-rs && cargo build --locked --target-dir "$target" -p k7-front --features test-hooks --bin k7-engine-testbed)
# Copied out: another branch's run may rebuild the shared one while these suites use it.
cp "$target/debug/k7-engine-testbed" ./k7-engine-testbed
# Run once, refused without its switch: macOS checks a new binary at its first start, which a suite's 10 s would pay.
./k7-engine-testbed 2> /dev/null || true
export ENGINE_CORE_BIN="$PWD/k7-engine-testbed" ENGINE_CORE_REQUIRED=1
# The application firewall drops a LAN peer's connection to the unsigned testbed: those two tests run on main's Linux gate.
export K7_TEST_NO_LAN_PEER=1
# The same set as `npm run ci:local`, so a ci/ branch's gate is what a developer runs before pushing (docs/release.md).
npm run ci:local
