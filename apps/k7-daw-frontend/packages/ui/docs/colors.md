# Colours

A track is coloured by its category, in Rust (`apps/k7-daw-wasm/song-schema/docs/trackColor.md`); this page is the palette for what the page tells apart by order.

## Series that are not tracks (`src/series-colors.ts`)

Things told apart by their ORDER rather than their kind — a lane's takes, a rack's slices, a
deck's cue buttons — share one palette of twelve hues at one OKLCH lightness and chroma, every one
at 6:1 or better on the app's ground, so none shouts over another and dark type reads on all of
them. `seriesColor(i)` walks it with a stride of five, so neighbours sit 150° apart and the first
eight keep off the amber the desk's readout uses. One palette makes the same index the same colour in every view: the third take
and the third slice are one colour.
