# Machine jobs

Some work happens on the machine your project plays on rather than in the page: a stem cut, a set list brought to the disk, a bounce. This page lists those jobs.

## Renders

Some commands bounce audio offline: freezing a track, consolidating, processing a clip through a chain, or a mixdown. A bounce runs through an engine binary: the one installed beside k7's file service, else the engine your project is bound to. The finished files are written into your workspace. Rendering a video's picture is a different job, done by k7's backend rather than by an engine; see the picture section of this guide.

## Jobs k7 asks of the engine's machine

When you open a project, k7 binds it to one engine. Some work happens on that engine's **machine**, because it needs that machine's disk, CPU or GPU. k7's backend asks the machine's supervisor, which on a device is its own, and files are copied as needed. You do not start these jobs by hand; the commands and buttons that need them do.

| Job | When it runs | What happens |
|---|---|---|
| **Stem separation** | Splitting a clip or a set list's records into stems, from a playlist or a chain that separates | The machine decodes the audio and runs the separation model on its own CPU or GPU, then writes the stems as 24-bit WAV files, which travel back. Asking for the same cut twice joins the job already running. |
| **Set list prefetch** | Every minute, while projects are open | The records your open projects' set lists name are copied onto the machine ahead of time, so a set list's records are on its disk before the gig, even if the venue has no network. A list that has not changed is not asked again. |
| **Render** | A bounce, as above | The machine runs its engine binary offline on the compiled project and sends back the mix (and stems when asked). |
| **The log** | The editor's **Logs** panel | The panel shows the engine machine's log, read through the tunnel. |

### Choosing what separation runs on

Stem separation runs on the CPU unless the machine is told otherwise. From the command bar, with a project open on that machine:

```text
system onnx dml,cpu
```

Providers are tried in order: `cpu`, `dml` (DirectML on Windows), `cuda`, `coreml`. An empty list goes back to the CPU. The choice is stored on the machine itself. (A command-line engine keeps it only until it stops; put `onnxProviders` in its config file to keep it.)
Two related commands choose which chain does the separating, for every surface that can start one:

| Command | What it stores |
|---|---|
| `system stems @k7-core/stems` | The chain that separates a record into stems |
| `system instrumental @k7-core/instrumental` | The chain that gives a record without its voice |

An empty reference clears either one.
