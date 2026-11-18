// @studio/ui/icons/all — every lucide icon by name, for an icon a person PICKS rather than one
// the code draws. Import it dynamically: it is ~1850 components, and only reaching them through this
// separate module keeps a static `@studio/ui/icons` import tree-shaken. See docs/icons.md.

// @ts-expect-error — the same components `lucide-react` exports, from a file with no .d.ts beside it.
export * from "lucide-react/dist/esm/icons/index.mjs";
