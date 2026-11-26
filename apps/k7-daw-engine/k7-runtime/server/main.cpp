// main.cpp — the k7-runtime executable: its argv goes to the Rust server, which runs a one-shot mode or serves.
// The modes and their arguments are docs/cliModes.md; argv is read in the Rust crate's src/cli.rs.

#include "../engine/capi/k7_server.h"

int main(int argc, char** argv) { return k7_server_main(argc, argv); }
