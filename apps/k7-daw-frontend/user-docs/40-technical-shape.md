# The shape of it

k7 is a folder of text, a service that reads it, and an engine that plays it.

The **folder** is the song: k7x files under a project directory, one entity per file. Nothing is hidden in a binary — a song diffs and greps like source, because it is. Projects live in **workspaces**, folders on the server that k7 keeps in step, live, on every device you open them on.

The **file service** (`apps/k7-daw-backend/packages/k7-file-service`) runs on your computer or phone, inside k7's Rust front, for its one user; a server runs none and only lists and serves its old workspaces for download. It owns the account's workspaces, reads and writes their files, compiles them, and answers the web app over HTTP.

The **engine** is a C++ binary (`apps/k7-daw-engine/k7-runtime`) built on JUCE + Tracktion, running on the machine with the audio interface. It knows nothing about k7x or about files: it receives a compiled `ProjectDoc` as JSON over a WebSocket and plays it. It is not k7' engine in particular — it is the machine's, and any app that can produce a `ProjectDoc` can drive it.

The **supervisor** (`apps/k7-daw-backend/packages/k7-supervisor`), beside it, starts and watches the engine, keeps it listed at the studios it is connected to, and holds the tunnel a server reaches the machine through. It runs from the command line with a config file, or under k7's backend on a computer (started by the desktop app) and inside the Android app, which drive it over its standard input and output.

The **web app** (`apps/k7-daw-frontend`) is k7' UI. It talks HTTP to the file service for anything on disk, and a WebSocket to the engine for anything live — transport, meters, plugin parameters — directly when the browser may reach the machine, relayed through the server otherwise. Those two channels are deliberately separate: one is durable state, the other is a 20 Hz feed nobody should persist.
