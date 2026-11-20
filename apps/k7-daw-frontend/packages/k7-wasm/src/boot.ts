// boot.ts — the one thing a host awaits before it mounts: the module's readiness.
// A reader would get wrong that nothing else needs it: a call made too early throws `unavailable`, never waits.

export { wasmReady } from "./abi.ts";
