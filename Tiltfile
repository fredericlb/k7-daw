# Tiltfile — the dev session: `npm run dev` (i.e. `tilt up`) starts every dev server as
# its own restartable resource, web UI at http://localhost:10350, instead of one TUI
# where a crash is all-or-nothing. The task list comes from scripts/tilt-tasks.mjs
# (derived from the workspace, so an app joins by existing). turbo keeps build/test —
# Tilt only owns `dev`. See docs/dev.md.

version_settings(check_updates=False)
analytics_settings(enable=False)

tasks = decode_json(str(local("node scripts/tilt-tasks.mjs", quiet=True)))

for t in tasks:
    local_resource(
        t["label"],
        serve_cmd="npm run dev",
        serve_dir=t["dir"],
        labels=[t["group"]],
        allow_parallel=True,
    )

# The Rust front on 8789: a cargo build, not a package's `dev` (docs/dev.md).
local_resource(
    "k7-front",
    serve_cmd="node scripts/dev-front.mjs",
    serve_dir="apps/k7-daw-backend",
    labels=["apps"],
    allow_parallel=True,
)
