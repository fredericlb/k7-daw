# The offline bounce

`take-render` renders a `ProjectDoc` to audio through this machine's engine binary in its one-shot
`--render` mode. Its callers are the engine's bench scripts under
`apps/k7-daw-engine/k7-runtime/scripts/`; the app's own renders run in the backend.

## Choices

| choice | why |
|---|---|
| every bounce takes a `Renderer` (`src/renderer.ts`): a folder in, the engine's manifest out | who renders belongs to the host; `localRenderer` runs this machine's binary |
| `renderProject` (`src/renderProject.ts`) names a missing engine out loud | a bare "render failed" reads as a broken document |
| a wedged bounce is cut after five minutes | a three-minute song renders in seconds, so a render past that bound is stuck |
| the scripts reach `renderProject` by path, not through `exports` | a script runs with `npx tsx` and is never imported by specifier (`docs/inventaire.md`) |
