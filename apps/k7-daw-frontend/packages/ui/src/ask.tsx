// ask.tsx — a confirmation or a one-field question, as a promise: the replacement for
// `window.confirm`, an OS box that ignores the app's Escape ladder, its focus and its language.
// Render <AskHost/> once beside <Toaster/>. Labels default to French; pass them in another
// language. See docs/ask.md.

import { useEffect, useRef, useState } from "react";

// The dialog's buttons are 44 px pills: glass for the secondary, accent for the primary, error-red for the one that does not come back.
const PILL =
  "type-label inline-flex h-11 cursor-pointer items-center justify-center rounded-full px-[18px] outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40";
const SECONDARY = `${PILL} glass text-foreground hover:brightness-125`;
const PRIMARY = `${PILL} bg-primary text-primary-foreground shadow-(--k7-highlight) hover:bg-primary-hover`;
const DANGER = `${PILL} bg-error text-ink hover:brightness-95`;

interface Common {
  title: string;
  /** The consequence, in the words the person needs — newlines survive, unlike in a native box. */
  body?: string;
  confirmLabel?: string;
  cancelLabel?: string;
}

export interface AskConfirm extends Common {
  kind: "confirm";
  /** Colour the confirm as destructive: this one does not come back. */
  danger?: boolean;
}

export interface AskText extends Common {
  kind: "text";
  /** What the field starts with — empty when there is nothing to suggest. */
  initial?: string;
  placeholder?: string;
  /** Why this value will not do, or "" when it will — re-asked per keystroke, so the button can refuse in place. */
  validate?: (value: string) => string;
}

type Pending = (AskConfirm | AskText) & {
  id: number;
  settle: (v: boolean | string | null) => void;
};

let nextId = 0;
let listener: ((p: Pending) => void) | null = null;

/** Yes or no, answering false when it is dismissed however that happens. */
export function askConfirm(q: Omit<AskConfirm, "kind">): Promise<boolean> {
  return new Promise((resolve) => {
    // No host mounted: the answer is NO, which is the safe half of every question this asks.
    if (!listener) return resolve(false);
    listener({ ...q, kind: "confirm", id: nextId++, settle: resolve as never });
  });
}

/** One field, answering null when cancelled — never "", which a caller cannot tell from an empty answer. */
export function askText(q: Omit<AskText, "kind">): Promise<string | null> {
  return new Promise((resolve) => {
    if (!listener) return resolve(null);
    listener({ ...q, kind: "text", id: nextId++, settle: resolve as never });
  });
}

/** Mount once, at the app root. One question at a time: a second would be a stack nobody asked for. */
export function AskHost() {
  const [pending, setPending] = useState<Pending | null>(null);
  useEffect(() => {
    listener = setPending;
    return () => {
      listener = null;
    };
  }, []);
  if (!pending) return null;
  // Keyed on the id so a second question opens with its own field rather than the last one's text.
  return (
    <AskDialog
      key={pending.id}
      pending={pending}
      onDone={(v) => {
        setPending(null);
        pending.settle(v);
      }}
    />
  );
}

function AskDialog({
  pending,
  onDone,
}: {
  pending: Pending;
  onDone: (v: boolean | string | null) => void;
}) {
  const text = pending.kind === "text" ? pending : null;
  const [value, setValue] = useState(text?.initial ?? "");
  const field = useRef<HTMLInputElement | null>(null);
  const invalid = text?.validate ? text.validate(value) : "";
  // A cancel is the same answer however it arrives — Escape, the backdrop, the button.
  const cancel = () => onDone(text ? null : false);

  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, []);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={pending.title}
      className="fixed inset-0 z-(--k7-z-dialog) flex items-center justify-center p-4"
      onKeyDown={(e) => {
        if (e.key === "Escape") cancel();
        if (e.key === "Enter" && !invalid) onDone(text ? value : true);
      }}
    >
      <button
        type="button"
        aria-label={pending.cancelLabel ?? "Annuler"}
        className="scrim absolute inset-0 animate-[k7-fade-in_var(--k7-dur-fade)_ease-out]"
        onClick={cancel}
      />
      <div className="panel-dialog relative flex max-h-full w-[min(var(--k7-dialog-w),92vw)] flex-col gap-3.5 overflow-y-auto rounded-(--k7-r-dialog) p-[22px] animate-[k7-dialog-in_var(--k7-dur-dialog)_var(--k7-ease-spring)]">
        <h2 className="type-sheet-title text-foreground">{pending.title}</h2>
        {pending.body ? (
          <p className="whitespace-pre-line text-sm leading-[19px] text-fg-2">
            {pending.body}
          </p>
        ) : null}
        {text ? (
          <div className="flex flex-col gap-1">
            <input
              ref={field}
              value={value}
              placeholder={text.placeholder}
              onChange={(e) => setValue(e.target.value)}
              className="type-body h-11 rounded-[14px] border border-(--k7-input-line) bg-(--k7-input-fill) px-3.5 text-foreground outline-none focus:border-(--k7-focus-line) focus:bg-(--k7-input-fill-focus) focus:shadow-(--k7-focus-ring)"
            />
            {invalid ? (
              <span className="type-status text-error">{invalid}</span>
            ) : null}
          </div>
        ) : null}
        <div className="flex items-center justify-end gap-2">
          <button type="button" onClick={cancel} className={SECONDARY}>
            {pending.cancelLabel ?? "Annuler"}
          </button>
          <button
            type="button"
            disabled={!!invalid}
            onClick={() => onDone(text ? value : true)}
            className={
              pending.kind === "confirm" && pending.danger ? DANGER : PRIMARY
            }
          >
            {pending.confirmLabel ?? "Confirmer"}
          </button>
        </div>
      </div>
    </div>
  );
}
