// copyText.ts — copies a line to the clipboard from a webview: the async API first, a selection and `execCommand` after.
// What a reader gets wrong: the fallback needs the text selected in a visible, readonly field, which iOS's webview requires.

/** Copies `text`; false when neither way worked, with `area` left selected for a hand copy. */
export async function copyText(
  text: string,
  area?: HTMLTextAreaElement | null,
): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // A webview without the permission falls through to the selection.
  }
  const field = area ?? document.createElement("textarea");
  if (!area) {
    field.value = text;
    field.readOnly = true;
    field.style.position = "fixed";
    field.style.opacity = "0";
    document.body.appendChild(field);
  }
  try {
    field.focus();
    field.select();
    field.setSelectionRange(0, text.length);
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    if (!area) field.remove();
  }
}
