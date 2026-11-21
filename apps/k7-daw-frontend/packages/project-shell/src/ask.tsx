// ask.tsx — a confirmation or a one-field question, as a promise. The shape `toast` already has,
// and no external library either, for the same reason it has none: this package draws before an
// app is set up. It exists because nine call sites had settled for `window.confirm`, an OS box
// that ignores the app's Escape ladder, its focus and its language. Render <AskHost/> once beside
// <Toaster/>. See docs/ask.md.

import { useEffect, useRef, useState } from "react";

const cx = (...parts: (string | false | undefined)[]): string =>
  parts.filter(Boolean).join(" ");

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
  /** Only to be read: one button, no cancel. Set by `askNotice`. */
  notice?: boolean;
}

export interface AskText extends Common {
  kind: "text";
  /** What the field starts with — empty when there is nothing to suggest. */
  initial?: string;
  placeholder?: string;
  /** Why this value will not do, or "" when it will — re-asked per keystroke, so the button can refuse in place. */
  validate?: (value: string) => string;
  /** Other things to do WITH the value, as buttons before the confirm — `askTextAction` only. */
  actions?: { key: string; label: string }[];
  /** Set by `askTextAction`: the answer carries which button gave it. */
  withAction?: boolean;
  /** A button that rewrites the field in place, also pressed by `key` anywhere in the dialog — docs/ask.md. */
  tool?: { label: string; key?: string; press: (value: string) => string };
}

export interface AskChoice extends Common {
  kind: "choice";
  /** The answers, as buttons in this order — the LAST is the default Enter takes. `confirmLabel` is unused. */
  choices: { key: string; label: string }[];
}

/** A text answer and the button that gave it — `"confirm"` for the main one and for Enter. */
export interface TextAnswer {
  value: string;
  action: string;
}

type Answer = boolean | string | TextAnswer | null;

type Pending = (AskConfirm | AskText | AskChoice) & {
  id: number;
  settle: (v: Answer) => void;
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

/** Something to read and acknowledge: one button, and no question for a Cancel to refuse. */
export async function askNotice(
  q: Omit<AskConfirm, "kind" | "notice" | "danger" | "cancelLabel">,
): Promise<void> {
  await askConfirm({ ...q, notice: true });
}

/** One field, answering null when cancelled — never "", which a caller cannot tell from an empty answer. */
export function askText(q: Omit<AskText, "kind">): Promise<string | null> {
  return new Promise((resolve) => {
    if (!listener) return resolve(null);
    listener({ ...q, kind: "text", id: nextId++, settle: resolve as never });
  });
}

/** One field and SEVERAL things to do with it: the value and which button was pressed, null when cancelled. */
export function askTextAction(
  q: Omit<AskText, "kind" | "withAction">,
): Promise<TextAnswer | null> {
  return new Promise((resolve) => {
    if (!listener) return resolve(null);
    listener({
      ...q,
      kind: "text",
      withAction: true,
      id: nextId++,
      settle: resolve as never,
    });
  });
}

/** One of several answers, as the chosen key — null when dismissed, never a default nobody picked. */
export function askChoice(
  q: Omit<AskChoice, "kind" | "confirmLabel">,
): Promise<string | null> {
  return new Promise((resolve) => {
    if (!listener) return resolve(null);
    listener({ ...q, kind: "choice", id: nextId++, settle: resolve as never });
  });
}

/** The buttons' words when a question brings none: the host's language, English until it passes its own. */
export interface AskLabels {
  cancel: string;
  confirm: string;
}

const ENGLISH: AskLabels = { cancel: "Cancel", confirm: "Confirm" };

/** Mount once, at the app root. One question at a time: a second would be a stack nobody asked for. */
export function AskHost({ labels }: { labels?: Partial<AskLabels> } = {}) {
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
      pending={{
        ...pending,
        cancelLabel: pending.cancelLabel ?? labels?.cancel ?? ENGLISH.cancel,
        confirmLabel:
          pending.confirmLabel ?? labels?.confirm ?? ENGLISH.confirm,
      }}
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
  onDone: (v: Answer) => void;
}) {
  const text = pending.kind === "text" ? pending : null;
  const choice = pending.kind === "choice" ? pending : null;
  const [value, setValue] = useState(text?.initial ?? "");
  const field = useRef<HTMLInputElement | null>(null);
  // With no field to focus, the dialog itself takes focus, or Escape and Enter would reach nothing.
  const box = useRef<HTMLDivElement | null>(null);
  const invalid = text?.validate ? text.validate(value) : "";
  // A cancel is the same answer however it arrives — Escape, the backdrop, the button.
  const cancel = () => onDone(text || choice ? null : false);
  const tool = text?.tool;
  // Not an updater: `press` keeps state of its own, and StrictMode runs an updater twice.
  const pressTool = () => tool && setValue(tool.press(value));
  // A text answer, wrapped with the button that gave it when the caller asked which.
  const answer = (action: string): Answer =>
    text?.withAction ? { value, action } : value;
  // What Enter answers: the field, the LAST choice, or yes.
  const accept = () =>
    onDone(
      text
        ? answer("confirm")
        : choice
          ? (choice.choices.at(-1)?.key ?? null)
          : true,
    );

  useEffect(() => {
    if (!field.current) return void box.current?.focus();
    field.current.focus();
    field.current.select();
  }, []);

  return (
    <div
      ref={box}
      tabIndex={-1}
      role="dialog"
      aria-modal="true"
      aria-label={pending.title}
      className="fixed inset-0 z-(--k7-z-dialog) flex items-center justify-center p-4"
      onKeyDown={(e) => {
        // The tool's key is the dialog's, not the field's nor the app's (Space would be the transport).
        if (tool?.key && e.key === tool.key) {
          e.preventDefault();
          e.stopPropagation();
          if (!e.repeat) pressTool();
          return;
        }
        if (e.key === "Escape") cancel();
        if (e.key === "Enter" && !invalid) accept();
      }}
    >
      <button
        type="button"
        aria-label={pending.cancelLabel ?? ENGLISH.cancel}
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
          {tool ? (
            <button
              type="button"
              // Focus stays in the field, so Enter after tapping still answers rather than pressing the tool again.
              onMouseDown={(e) => e.preventDefault()}
              onClick={pressTool}
              className={cx(SECONDARY, "mr-auto")}
            >
              {tool.label}
            </button>
          ) : null}
          {pending.kind === "confirm" && pending.notice ? null : (
            <button type="button" onClick={cancel} className={SECONDARY}>
              {pending.cancelLabel ?? ENGLISH.cancel}
            </button>
          )}
          {text?.actions?.map((a) => (
            <button
              key={a.key}
              type="button"
              disabled={!!invalid}
              onClick={() => onDone(answer(a.key))}
              className={SECONDARY}
            >
              {a.label}
            </button>
          ))}
          {choice ? (
            // Every answer a button; the last one is the primary, as Enter takes it.
            choice.choices.map((c, i) => (
              <button
                key={c.key}
                type="button"
                onClick={() => onDone(c.key)}
                className={
                  i === choice.choices.length - 1 ? PRIMARY : SECONDARY
                }
              >
                {c.label}
              </button>
            ))
          ) : (
            <button
              type="button"
              disabled={!!invalid}
              onClick={accept}
              className={
                pending.kind === "confirm" && pending.danger ? DANGER : PRIMARY
              }
            >
              {pending.confirmLabel ?? ENGLISH.confirm}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
