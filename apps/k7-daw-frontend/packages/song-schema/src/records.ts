// records.ts — the drag a set list's row carries: the browser's own drag-and-drop payload, which no Rust side shares.
// The rules on a record (its path, resolution, names) are `@studio/k7-wasm/records`; see docs/playlists.md.

/** Our own MIME, so a deck accepts a playlist row and refuses every other thing a browser can drop. */
export const RECORD_DRAG_TYPE = "application/x-studio-record";

/** What travels is the record's path: the deck finds it in its own set list. */
export interface RecordDrag {
  record: string;
}

export const packRecordDrag = (d: RecordDrag): string => JSON.stringify(d);

/** …and back, refusing anything that is not one — a drop carries whatever the page it came from wrote. */
export function parseRecordDrag(text: string): RecordDrag | null {
  try {
    const v = JSON.parse(text) as Partial<RecordDrag>;
    return typeof v?.record === "string" && v.record
      ? { record: v.record }
      : null;
  } catch {
    return null;
  }
}
