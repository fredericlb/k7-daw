# Chains — what an engine separates with, named on the engine

A cut runs a **chain**: which module separates a mix and which model it loads. That is a fact about the
machine that runs it — its CPU, its GPU, the models it has downloaded — so the standing answer is the
machine's own, in `stems` / `instrumental` in its `engine-host.json`
(`apps/k7-daw-engine/k7-supervisor/docs/supervisor.md`), and it travels back on its heartbeat as `PublicEngine.chains`
(`heartbeat.md`). One machine separating with Demucs and another with something smaller is a normal estate.

```
GET  /api/engines/:id/chains           →   { chains: { stems?, instrumental? } }
PUT  /api/engines/:id/chains           { stems?, instrumental? }  →  the chains as they then stand
```

**The write is the device's own engine's**, as everything else about an engine is (`ownership.md`): another engine gets
a 403, and an engine the registry does not hold is a 404. Naming neither key is a no-op that answers with
what the machine already holds, so a card saving one field cannot blank the other, and `""` is how a chain
is unsaid — the key then leaves the file rather than standing there empty.

**The value is not kept here.** The route opens the engine's jobs channel and asks its supervisor
(`apps/k7-daw-frontend/packages/engine-tunnel/docs/jobs.md`, the `chains` job), which writes its own settings and answers with
them; the front answers that. So an engine that is not connected has nothing to name — a 503 saying so —
and a changed chain reads differently a few seconds later, on the heartbeat that follows.

On the app's side the form is the **Chains tab of that machine's engine page**
(`apps/k7-daw-frontend/src/settings/ChainsSection.tsx`), which reads the library's cold chains as its options
and this route as the answer: the setting is not a user preference, so it is not on `/settings`.

