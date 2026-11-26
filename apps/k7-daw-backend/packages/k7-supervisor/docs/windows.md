# Windows

What only Windows does for the supervisor lives in `src/win`. The decisions are plain code that runs and is
tested everywhere (`tests/all/windows.rs`); the calls that ask Windows are `cfg(windows)` and are checked by
`scripts/agent/win-clippy.sh`, and run only on a Windows machine.

| choice | what forced it |
|---|---|
| the host's job is created by the supervisor, its handle duplicated into the host BEFORE ours closes, then joined | closing the last handle of a kill-on-close job ends the job and the supervisor with it; every process started later, the engine included, dies with the host |
| a job that cannot be made kill-on-close is reported, not ignored | a job without the flag protects nothing, and the host believes it does |
| the app's three standard handles are made non-inheritable at start | a child holding a copy of the app's pipe costs the app our EOF |
| a port is bound with `SO_EXCLUSIVEADDRUSE`, through Winsock | a plain Windows bind succeeds beside a listener that let its port be shared, which the engine's own bind would not |
| out of descriptors is WSAEMFILE and WSAENOBUFS | the probe's third answer, "could not ask", must never read as "down" |
| the listener of a port is read from the TCP table (`GetExtendedTcpTable`, listeners, IPv4) | there is no `lsof`; a row's port is a network-order `u16` in a `u32` |
| the hostname is the wide DNS name, as UTF-8 | `gethostname` answers in the ANSI code page, which JSON refuses |
| adapters come from `GetAdaptersAddresses`, up ones only; memory from `GlobalMemoryStatusEx` | the LAN addresses, the MAC fingerprint and the performance score read the same fields as on the other systems |
| trust roots are the user's and the machine's ROOT stores walked by hand, not `rustls-native-certs` | the `Disallowed` stores and the server-auth usage are applied; a root naming no usage is good for every use, one naming usages without server auth is refused |
| DirectML runs on the first non-software adapter with 1 GiB of dedicated memory, by its DXGI index; a machine with only integrated adapters refuses | an integrated GPU shares the CPU's memory and slows the machine; DXGI's order is ONNX Runtime's index |
| the adapter walk lists nothing: ONNX Runtime's own GPU filter decides | a DXGI that cannot start must not take the GPU away |
| a crash leaves `crash-supervisor-<pid>.dmp` in the state folder (`win/crash.rs`), written by DbgHelp once, from the first of the panic hook, a SIGABRT handler and the unhandled-exception filter, on a thread of its own | an app restarts a dead supervisor before anyone looks, and an exit code of `0xC0000409` says only "aborted"; the C runtime's `abort()` (ONNX Runtime's, DirectML's) raises SIGABRT before failing fast, which no exception filter sees, and the faulting thread's own stack is not captured reliably from itself. Rust's own abort fails fast with no handler at all |
