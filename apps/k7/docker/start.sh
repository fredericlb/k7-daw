#!/bin/sh
# start.sh — the image's two processes: node's backend on 127.0.0.1:8790, and k7's Rust front on 8789 in front
# of it, which is what Traefik and the engine reach. Either ending ends the container, so its restart policy
# brings both back; a stop is passed to both and waited for (docs/images.md).
chmod 700 /data /library /workspaces 2>/dev/null
# Drawn at each start unless the deployment sets one: only these two processes ever hold it.
: "${K7_FRONT_SECRET:=$(od -An -tx1 -N32 /dev/urandom | tr -d ' \n')}"
export K7_FRONT_SECRET
# The engine core and the file hub are the front's: node has none, so no other value is honoured.
export K7_ENGINE_CORE=front K7_FILES=front
# The backend's last routes in the front, the page from the image's built frontend; node keeps running behind it (docs/images.md).
export K7_LAST_ROUTES=front K7_FRONTEND_DIST=/app/apps/k7/frontend/dist
PORT=8790 STUDIO_LISTEN_HOST=127.0.0.1 K7_FRONT_PORT=8789 node /app/server-dist/backend.mjs &
backend=$!
K7_FRONT_LISTEN=0.0.0.0:8789 K7_FRONT_UPSTREAM=127.0.0.1:8790 k7-front &
front=$!
trap 'kill -TERM "$front" "$backend" 2>/dev/null; wait' TERM INT
wait -n
code=$?
# Which one ended, and how: a code past 128 is the signal that killed it (docs/images.md).
if kill -0 "$front" 2>/dev/null; then who=backend; else who=k7-front; fi
if [ "$code" -gt 128 ]; then echo "start.sh: $who exited, killed by signal $((code - 128))" >&2
else echo "start.sh: $who exited with code $code" >&2; fi
kill -TERM "$front" "$backend" 2>/dev/null
wait
exit "$code"
