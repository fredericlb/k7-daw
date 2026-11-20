# Icons — one set, from `@studio/ui/icons`

Every icon in k7 is a [lucide](https://lucide.dev) icon, imported from
`@studio/ui/icons`:

```tsx
import { Settings, Trash2 } from "@studio/ui/icons";

<Button variant="ghost" size="sm" aria-label="Settings">
  <Settings className="size-4" />
</Button>
```

## Why one set

An emoji renders at its own font's size, colour and baseline, and a text glyph such as `⚙` or
`🗑` renders differently again on every platform. A bar mixing the two never lines up, and two apps
drawing the same action with different pictures do not read as one family. A single stroke
icon set takes the text colour, scales with a class, and looks the same everywhere.

## Why through `@studio/ui`

The package owns the set, so the choice is made once. An app never
lists `lucide-react` itself; the shared biome config warns on a direct import
(`style/noRestrictedImports` in `biome.json`).

## Sizes

- `size-4` (16 px) is the default: buttons, header entries, menu items.
- `size-3.5` for dense text (`text-xs` links, inline badges), `size-3` inside a meta line.
- `size-5` next to a heading.

Colour comes from `currentColor`; a filled state (a favourite star, a recording dot) adds
`fill-current`.

## Alignment

`theme.css` puts `svg.lucide` on the text baseline, so an icon inside a plain `<p>`, `<a>` or
legacy button sits on the line without a flex wrapper. Inside a flex parent the rule is inert.

## What stays text

Glyphs that are **content**, not pictures of an action: note names (`C♯`, `B♭`), a multiplier
(`×2`), an arrow inside a sentence, a password mask. And anything drawn on a `<canvas>`, where
an SVG component cannot go.

## An icon chosen by NAME: `@studio/ui/icons/all`

An icon a person picks — a menu entry's, a command's — is not known when the code is written, so it
cannot be imported by name. Reading the whole `@studio/ui/icons` namespace to look one up keeps
every icon in whichever chunk holds the static imports, which is the first screen's: about 190 KB
gzipped to draw a few dozen.

`@studio/ui/icons/all` is the same components through lucide's per-icon index, a module of its
own. Import it with `import()`, at the moment a name has to be resolved: the bundle then splits it
into a chunk loaded on demand, and the static `@studio/ui/icons` imports tree-shake back to the
icons the code actually draws.

