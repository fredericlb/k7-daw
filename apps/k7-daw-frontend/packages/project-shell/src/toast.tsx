// toast.tsx — lightweight notifications, shared by the apps that mount this shell. No external
// library. Render <Toaster/> once at the app root; call `showToast({ message, variant })` from
// anywhere. A SUCCESS leaves after `duration`, a WARNING after a longer one; an ERROR stays until
// dismissed, and hovering or focusing any of them holds it. A NEUTRAL toast with progress stays until the code
// dismisses it. The only toast of studio: patch it here — docs/toast.md.

import { Button } from "@studio/ui/button";
import { CheckCircle2, TriangleAlert, X, XCircle } from "@studio/ui/icons";
import { Progress } from "@studio/ui/progress";
import { Spinner } from "@studio/ui/spinner";
import { type ReactNode, useEffect, useState } from "react";

const cx = (...parts: (string | false | undefined)[]): string =>
  parts.filter(Boolean).join(" ");

export interface ToastAction {
  label: string;
  onClick: () => void;
}

/** `value` of `total` (100 when omitted): drawn as a 6 px bar, and as "value / total" in mono when `total` is given. */
export interface ToastProgress {
  value: number;
  total?: number;
}

export interface ToastItem {
  id: number;
  message: string;
  /** A bold line over the message. */
  title?: string;
  variant: ToastVariant;
  /** Milliseconds before it leaves on its own, or 0 to stay until dismissed. */
  duration: number;
  /** Bottom-right by default; top-right or top-centre for what must not be missed behind the transport bar. */
  placement: ToastPlacement;
  /** Replaces the variant's icon (an engine toast draws an unplugged cable). */
  icon?: ReactNode;
  /** A 36 px pill after the text (Restart); the toast stays for it until the duration ends. */
  action?: ToastAction;
  progress?: ToastProgress;
  /** False hides the close button: the toast leaves when the code says so (the engine link is down). */
  dismissible?: boolean;
}

export type ToastPlacement = "bottom-right" | "top-right" | "top-centre";

/** A warning is something that worked, but not quite as asked: worth reading, not worth blocking on. */
export type ToastVariant = "success" | "warning" | "error" | "neutral";

/** How long a success stays on screen; an error gets 0, meaning it waits to be dismissed. */
export const TOAST_DURATION_MS = 5000;
/** A warning is a sentence to read, not a confirmation to glance at. */
export const WARNING_DURATION_MS = 8000;

type Patch = Partial<
  Pick<ToastItem, "message" | "title" | "progress" | "variant" | "action">
>;
type Control =
  | { kind: "update"; id: number; patch: Patch }
  | { kind: "dismiss"; id: number };

let nextId = 0;
const listeners = new Set<(t: ToastItem) => void>();
const controls = new Set<(c: Control) => void>();

/** Hear every toast as it is shown — an app keeping a log of what it said; returns the unsubscribe. */
export function onToast(listener: (t: ToastItem) => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function defaultDuration(variant: ToastVariant, hasProgress: boolean): number {
  // An error nobody read is an error that did not happen, so it waits to be dismissed.
  if (variant === "error") return 0;
  if (variant === "warning") return WARNING_DURATION_MS;
  if (variant === "neutral" && hasProgress) return 0;
  return TOAST_DURATION_MS;
}

/** Shows a toast and answers its id, for `updateToast` and `dismissToast`. */
export function showToast(opts: {
  message: string;
  title?: string;
  variant?: ToastVariant;
  duration?: number;
  placement?: ToastPlacement;
  icon?: ReactNode;
  action?: ToastAction;
  progress?: ToastProgress;
  dismissible?: boolean;
}): number {
  const variant = opts.variant ?? "success";
  const t: ToastItem = {
    id: nextId++,
    message: opts.message,
    title: opts.title,
    variant,
    duration: opts.duration ?? defaultDuration(variant, !!opts.progress),
    placement: opts.placement ?? "bottom-right",
    icon: opts.icon,
    action: opts.action,
    progress: opts.progress,
    dismissible: opts.dismissible,
  };
  for (const l of listeners) l(t);
  return t.id;
}

/** Change a toast still on screen: a scan moving from 34 to 35 of 120. */
export function updateToast(id: number, patch: Patch): void {
  for (const c of controls) c({ kind: "update", id, patch });
}

/** Make a toast leave: the end of a scan. */
export function dismissToast(id: number): void {
  for (const c of controls) c({ kind: "dismiss", id });
}

/** `dismissLabel` is the close button's accessible name, in the host app's language. */
export function Toaster({
  dismissLabel = "Dismiss",
  regionLabel = "Notifications",
  phone,
}: {
  dismissLabel?: string;
  /** The landmark's accessible name that holds every toast. */
  regionLabel?: string;
  /** The folded shell's answer; left out, the stacks follow the viewport (under 640 px wide is a phone). */
  phone?: boolean;
} = {}) {
  const [items, setItems] = useState<ToastItem[]>([]);

  useEffect(() => {
    const add = (t: ToastItem) => setItems((prev) => [...prev, t]);
    const control = (c: Control) =>
      setItems((prev) =>
        c.kind === "dismiss"
          ? prev.filter((t) => t.id !== c.id)
          : prev.map((t) => (t.id === c.id ? { ...t, ...c.patch } : t)),
      );
    listeners.add(add);
    controls.add(control);
    return () => {
      listeners.delete(add);
      controls.delete(control);
    };
  }, []);

  const dismiss = (id: number) =>
    setItems((prev) => prev.filter((t) => t.id !== id));

  const stack = (placement: ToastPlacement, className: string) => {
    const here = items.filter((t) => t.placement === placement);
    return here.length ? (
      <div
        className={cx(
          "pointer-events-none fixed flex flex-col gap-2 z-(--k7-z-toast)",
          className,
        )}
      >
        {here.map((t) => (
          <Toast
            key={t.id}
            item={t}
            dismissLabel={dismissLabel}
            onDismiss={() => dismiss(t.id)}
          />
        ))}
      </div>
    ) : null;
  };

  // Phone: left and right 16, the bottom stack 140 above the edge (over the bar), the top ones under the header.
  const bottomPhone =
    "left-4 right-4 bottom-(--k7-toast-bottom-phone) items-stretch";
  const bottomWide =
    "right-(--toast-right,1rem) bottom-(--toast-bottom,var(--k7-panel-bottom)) w-[min(var(--k7-toast-max-w),calc(100vw-2rem))] items-stretch";
  const topPhone = "left-4 right-4 top-(--k7-toast-top-phone) items-stretch";
  return (
    <div role="region" aria-label={regionLabel} className="contents">
      {stack(
        "top-right",
        phone === true
          ? topPhone
          : cx(
              "right-(--toast-right,1rem) top-(--toast-top,calc(var(--k7-header-h)+8px)) w-[min(var(--k7-toast-max-w),calc(100vw-2rem))] items-stretch",
              phone === undefined &&
                "max-sm:left-4 max-sm:right-4 max-sm:top-(--k7-toast-top-phone) max-sm:w-auto",
            ),
      )}
      {stack(
        "bottom-right",
        phone === true
          ? bottomPhone
          : cx(
              bottomWide,
              phone === undefined &&
                "max-sm:left-4 max-sm:right-4 max-sm:bottom-(--k7-toast-bottom-phone) max-sm:w-auto",
            ),
      )}
      {/* The plugin-scan kind of toast: centred at the top, above every layer, sliding down from above. */}
      {stack(
        "top-centre",
        cx(
          "left-1/2 top-(--toast-top-centre,calc(var(--k7-header-h)+8px)) w-[min(var(--k7-toast-max-w),calc(100vw-2rem))] -translate-x-1/2 items-stretch !z-(--k7-z-system-toast)",
          phone === true && "!top-(--k7-toast-top-phone)",
          phone === undefined && "max-sm:!top-(--k7-toast-top-phone)",
        ),
      )}
    </div>
  );
}

const LOOK = {
  success: { Icon: CheckCircle2, iconCls: "text-ok", line: "border-white/18" },
  warning: {
    Icon: TriangleAlert,
    iconCls: "text-warn",
    line: "border-(--k7-warn-toast-line)",
  },
  error: {
    Icon: XCircle,
    iconCls: "text-error",
    line: "border-(--k7-error-toast-line)",
  },
  neutral: {
    Icon: CheckCircle2,
    iconCls: "text-primary-text",
    line: "border-white/18",
  },
} as const;

function Toast({
  item,
  dismissLabel,
  onDismiss,
}: {
  item: ToastItem;
  dismissLabel: string;
  onDismiss: () => void;
}) {
  // Hovering or focusing holds the toast: reading a message must not be a race against its own timer.
  const [held, setHeld] = useState(false);

  useEffect(() => {
    if (!item.duration || held) return;
    const timer = setTimeout(onDismiss, item.duration);
    return () => clearTimeout(timer);
  }, [item.duration, held, onDismiss]);

  const { Icon, iconCls, line } = LOOK[item.variant];
  const alert = item.variant === "error" || item.variant === "warning";
  const p = item.progress;
  const total = p?.total ?? 100;
  // A scan the code owns has no Dismiss: it leaves when the code says so.
  const dismissible = item.dismissible ?? !(item.variant === "neutral" && p);

  return (
    <div
      // Assertive for an error and a warning, which interrupt; polite for the rest, which merely inform.
      role={alert ? "alert" : "status"}
      aria-live={alert ? "assertive" : "polite"}
      onPointerEnter={() => setHeld(true)}
      onPointerLeave={() => setHeld(false)}
      onFocus={() => setHeld(true)}
      onBlur={() => setHeld(false)}
      className={cx(
        "float pointer-events-auto flex flex-col gap-2 rounded-(--k7-r-toast) py-2 pr-1 pl-3.5 text-foreground",
        line,
        item.placement === "top-centre"
          ? "animate-[k7-toast-in_var(--k7-dur-toast)_var(--k7-ease-lozenge)]"
          : "animate-in fade-in slide-in-from-bottom-4 duration-300",
      )}
    >
      <div className="flex items-center gap-2.5">
        {item.icon ? (
          <span
            className={cx(
              "grid shrink-0 place-items-center [&_svg]:size-[22px]",
              iconCls,
            )}
          >
            {item.icon}
          </span>
        ) : p ? (
          <Spinner className="size-[22px]" />
        ) : (
          <Icon className={cx("size-[22px] shrink-0", iconCls)} aria-hidden />
        )}
        <div className="min-w-0 flex-1 py-1">
          {item.title ? (
            <p className="type-label text-foreground">{item.title}</p>
          ) : null}
          <p
            className={cx(
              "type-secondary",
              item.title ? "text-fg-2" : "text-foreground",
            )}
          >
            {item.message}
            {p?.total ? (
              <span className="type-mono-value ml-2 text-fg-2">
                {Math.round(p.value)} / {p.total}
              </span>
            ) : null}
          </p>
        </div>
        {item.action ? (
          <Button size="sm" onClick={item.action.onClick}>
            {item.action.label}
          </Button>
        ) : null}
        {dismissible ? (
          <button
            type="button"
            onClick={onDismiss}
            aria-label={dismissLabel}
            title={dismissLabel}
            className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full text-fg-2 outline-none transition-colors hover:bg-on-fill hover:text-foreground focus-visible:shadow-(--k7-focus-ring)"
          >
            <X className="size-[18px]" strokeWidth={2.2} />
          </button>
        ) : null}
      </div>
      {p ? (
        <div className="pr-3.5 pb-1.5">
          <Progress
            value={(p.value / total) * 100}
            label={item.title ?? item.message}
          />
        </div>
      ) : null}
    </div>
  );
}
