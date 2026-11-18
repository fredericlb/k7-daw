# Choosing a record

**You click the centre of the vinyl.** The middle of a record is where its name is written, so
that is where you press to change it. The record itself stays `pointer-events-none`: only the
label takes the click, which leaves the outer groove free for grabbing it to mean scratching. A
desk too short for the record keeps the label alone, as a button of the same name
([deskLayout.md](deskLayout.md)).

A box opens, holding the records **of the set** — not every audio file of the workspace. It is
the **playlist the surface names** that says what is within reach tonight; searching hundreds of
files when the set holds twelve is searching the wrong list.

That is a parameter of `surfaces/<desk>.k7x` — `playlist: ete-2026`, a name under `_playlists/`
in the workspace — and the playlists themselves are written in k7, in the playlist pane. See
[setList.md](setList.md) for the why, and `packages/song/docs/playlists.md` for the format.

The desk does not write them: from here you **load** a record, you do not build the setlist. The
two gestures happen at different moments — one at home, one in the booth — and an "add" button
next to a crossfade is a button pressed by mistake.

## The sort: folder, then name

A row shows the file's name, with the **folder** holding it above and smaller. The list is sorted
by that folder, then by name, so that is the column the eye follows down. `localeCompare` in
French, not code-point order: `Alizée` files between `Alain` and `Daft`, where a raw comparison
puts every accented name after `Z`. `numeric` while we are here, so `Track 2` precedes
`Track 10`.

## The search

It filters the **whole list**, never what is on screen. Two things make it usable at a desk:

- **Accents are folded.** `alizee` finds `Alizée`, `rose` finds `Rosé`. Nobody composes an é
  mid-mix, and without this half the set becomes unreachable.
- **Terms count in any ORDER**, across folder and name: `bashung nuit` finds `Bashung/La nuit je mens.mp3`
  without knowing which part carries which word. A plain substring over the two parts joined
  fails on that, and searching part by part fails twice.

`Enter` loads the first **playable** result — not simply the first, so a keystroke aimed at the
row below does not load a record the workspace lacks.

**The box opens focused on the search only where the primary pointer is fine.** On a phone,
focusing a field raises the keyboard, which takes half the screen and squeezes the box to its
header: the list the box was opened for is then too short to tap a row in. A set is a dozen
songs, so a thumb picks from the list and opens the keyboard only by touching the search.

## What is not loadable stays visible

A record the workspace does not hold is shown, greyed, with its download icon and the reason in a
tooltip. The states are the `SetList`'s (see [setList.md](setList.md)): not in the workspace, no
engine. Hiding it would make a set missing three records indistinguishable from a set that never
had them.

## One box for both decks

`browsing` holds the deck concerned, or `null`. Two dialogues would be two states able to
contradict each other about which is open. The search is cleared **on the way out** rather than
on the way in: reopening for the other deck must start clean, and a search left behind would hide
the set behind someone's last word.

It is also the desk's only list: the middle belongs to the decks, which want the width
([mix.md](mix.md)).
