# Behaviour on Radix

The kit's interactive components take their behaviour from Radix primitives and keep only the look (classes, tokens, the sliding pill). Each line says what upstream owns and why we do not hand-write it.

- **`side-panel.tsx`**: Radix Dialog. `Sheet` is modal (focus trap, focus back to the opener, scroll lock, Escape); `SidePanel` is non-modal and ignores outside interaction, since people work in the panes while it is open. Both render in place, not in a portal, because the host positions them under the header. `SidePanel` keeps `role="complementary"`; the host's `onEscapeKeyDown` decides when the app's Esc ladder, not the panel, takes the key.
- **`picker.tsx`**: Radix Popover, not Select: Select's items cannot hold a search field, and the list must keep a name plus a path per row. Popover brings collision-aware placement, Escape and outside-click dismissal and focus return; the list's roving focus and the `listbox`/`option` roles stay ours.
