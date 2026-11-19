#!/bin/sh
# start.sh — the image's one process: k7's Rust front on 8789, which is what Traefik and the engine reach. A stop is
# passed to it and waited for (docs/images.md).
chmod 700 /data /library /workspaces 2>/dev/null
# The engine core and the file hub are the front's own: no other value is honoured.
export K7_ENGINE_CORE=front K7_FILES=front
K7_FRONT_LISTEN=0.0.0.0:8789 exec k7-front
