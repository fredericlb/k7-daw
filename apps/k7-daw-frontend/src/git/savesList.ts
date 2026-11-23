// savesList.ts — "Saves" in the git menu: the History panel opens on its Saves list, whether it was open or not.
// What a reader gets wrong: the request waits for a panel that mounts after it, so the order of the two acts does not matter.

let asked = false;
const listeners = new Set<() => void>();

/** Ask the History panel to show the Saves list. */
export function askSavesList(): void {
  asked = true;
  for (const l of listeners) l();
}

/** Takes the request once: true when the panel should switch to Saves. */
export function takeSavesList(): boolean {
  const was = asked;
  asked = false;
  return was;
}

export function onSavesList(fn: () => void): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}
