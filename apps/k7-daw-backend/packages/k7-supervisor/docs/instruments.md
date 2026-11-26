# instruments.md — the sampled instruments, in the Rust supervisor

The rules a musician sees (nothing downloaded until asked for, one at a time, pinned by hash, installed only
when whole, a cut download resumed) are the C++ supervisor's `docs/instruments.md`, unchanged. These are the
choices the Rust port makes.

| choice | why |
|---|---|
| the catalogue, the disk store and the install are `src/instruments/`; the queue, the rows and the commands are the part `parts/instruments` | the store and the install are blocking or pure and testable with no core; the part holds only state |
| the download is `src/download.rs`, over `http::client` | `http::send` returns a whole body as text, which a gigabyte archive cannot be; the client is built per hop, so each redirect is a fresh connection |
| redirects are followed (ten at most), never from https down to http; the `Range` is sent again on each hop | codeload and the mirrors redirect; the hash would catch a swapped file, but a downgrade is refused before it |
| the answer's status says what the `.part` becomes: 206 from its length appends, 200 replaces, 416 naming its length means whole, anything else drops it and starts once more | a server that cannot resume (GitHub's generated zips) must still land; a second dropped prefix is an error, never a loop |
| the timeout bounds the connect, the wait for an answer and each wait between chunks, 60 s each | a stalled line fails and resumes next time; the C++ bound the whole request at an hour, which a slow phone line would hit |
| a `.part` is written as the answer arrives, since the status is known first | the C++ client handed over the body before the status, so it wrote beside the `.part` until it knew; the error page never lands here either |
| `zip` unpacks, a name is checked on its own, backslashes read as separators | a name that climbs out is refused before anything is written, whatever platform made the archive |
| the part keeps its OWN copy of what the machine keeps, and asks the core to save it | the core's copy follows a tick behind a change it was asked for, so a second command would otherwise read a stale list |
| the first boot settles the choice in the part's boot, which the core awaits before the engine starts | the settings are saved before anything can read them |
| the fetch of what is kept starts at the first engine up, not at boot | the registry part measures the machine at boot and a download would spoil the measure; a machine whose engine cannot start fetches nothing until it can |
| a worker is aborted when the supervisor goes | a transfer outlives no supervisor; the `.part` it leaves is what the next start resumes |
