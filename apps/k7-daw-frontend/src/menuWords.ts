// menuWords.ts — Rust's `Text` turned into words: a literal as it is, a message key through the page's paraglide catalogue.
// A reader would get wrong that a missing key is silent: it reads as the key itself, and a test resolves every key Rust can emit.

import type { Text } from "@studio/k7-wasm/menu";
import { m } from "./paraglide/messages.js";

type Message = (params?: Record<string, unknown>) => string;
const messages = m as unknown as Record<string, Message | undefined>;

/** Whether the page's catalogue holds this message. */
export const hasMessage = (key: string): boolean => key in messages;

/** The words of a `Text`: its prefix, then its literal or its message with its parameters. */
export const words = (t: Text): string =>
  (t.prefix ?? "") +
  (t.key
    ? (messages[t.key]?.(t.params ?? undefined) ?? t.key)
    : (t.text ?? ""));
