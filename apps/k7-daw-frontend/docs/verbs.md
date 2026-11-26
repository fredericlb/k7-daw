# The verbs from the page

The page runs every command line and editing gesture through the backend's `POST /verb`, completes the command bar
through `POST /verb/complete`, reads the picture through `POST /picture` and `POST /picture/scene`, and imports a foreign
project through `POST /import` (`docs/verbs-port.md` at the root, `apps/k7-daw-backend/docs/file-changes.md` the
routes). The page holds no project and runs no verb of its own. This page holds the choices of the page's side; the code
is `src/verbs/`.

## Choices

| choice | why |
|---|---|
| one seam, `verbsHost()` of `@studio/project-shell`: the command bar, a gesture, a menu and the chain builder all send a line through `projectApi.command` | a button never calls an edit of its own, so every edit is a line the history names |
| the answer's lines are its log, then its refusal, then an escaped throw as `error: …` | a refusal reads after what the verb said before it |
| a run past a second is listed in the process list | a render or a separation takes as long as it takes, and the request stays open until it lands |
| the catalogue is asked of `GET /verb/catalogue` with the first palette | the crate builds it from `commands.json`, so the page parses no YAML and keeps no copy of the provider table |
| a completion that fails answers no items | the palette keeps asking, and a command bar never shows an error for a keystroke |
| a completion and the picture are POSTed without the write fence | they change no file, so the reads around them stay shared |
| the picture's document and its warnings are one answer; a picture that does not build is `{error}`, shown as the viewport's sentence | the compile's refusal is what the person needs to read, never a failed request |
| the viewport asks its scene of the backend once per picture document and armed lane, and keeps the scene object when the answer is unchanged | one builder draws both the preview and an export, and a new scene rebuilds a GL context and a `<video>` per layer |
| a dropped project file is posted as its bytes, its name and folder in headers | the home page has no open project, and a file name is no personal data to put in a URL |
