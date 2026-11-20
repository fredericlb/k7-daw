# Toasts

`src/toast.tsx` is the toast every studio app shows: `<Toaster/>` mounted once at the root,
`showToast()` called from anywhere.

It adds two things a plain toast lacks:

- a **warning** variant, which leaves on its own like a success, after a longer delay;
- a **placement**, top-right for what must not be missed behind the transport bar.

The bottom stack sits at `--toast-bottom`, above the app's bar. An app sets `--toast-top` (and
`--toast-bottom: auto`) where something of its own opens just above that bar: k7's folded shell opens
its Menu there, and a toast left on it covers the Menu's buttons.
