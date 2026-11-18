# Toasts

`src/toast.tsx` is the toast every studio app shows: `<Toaster/>` mounted once at the root,
`showToast()` called from anywhere.

## It is a fork of `@studio/ui/toast`, and the one to patch

`@studio/ui` ships the same design (`packages/ui/src/toast.tsx`), but nothing in studio imports
it. This copy adds two things the shared one lacks:

- a **warning** variant, which leaves on its own like a success, after a longer delay;
- a **placement**, top-right for what must not be missed behind the transport bar.

The bottom stack sits at `--toast-bottom`, above the app's bar. An app sets `--toast-top` (and
`--toast-bottom: auto`) where something of its own opens just above that bar: k7's folded shell opens
its Menu there, and a toast left on it covers the Menu's buttons.

A fix to how toasts behave in studio goes here, not in `@studio/ui/toast`, where it would change
nothing a user sees; the two copies are expected to drift.
