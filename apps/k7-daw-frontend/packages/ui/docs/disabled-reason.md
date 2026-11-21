# A disabled control says why

`Button`, `CapsuleButton` and `DropdownMenuItem` take `disabledReason`. A control that is disabled
with a reason is not natively disabled: it carries `aria-disabled`, stays focusable and pressable, and
a press shows the reason through `setDisabledReasonSink` (the host wires it to a toast,
`src/disabledReasonToast.ts`).

## Choices

- **The reason is its own prop, not the `title`.** A title is often just the label, and a press on a
  disabled control must not toast its name. A caller states the reason or gets the plain `disabled`.
- **The kit owns it, the host owns the toast.** `@studio/ui` cannot import the shell's toast (the
  toast imports the kit), so the kit only calls the sink.
- **A native `disabled` takes no press, no focus and no tooltip.** Touch has no hover at all, so a
  reason in a tooltip never reaches the people who need it most.
- **A menu trigger that is blocked passes `disabled` to the Radix trigger too**, so the menu does not
  open under the toast.
