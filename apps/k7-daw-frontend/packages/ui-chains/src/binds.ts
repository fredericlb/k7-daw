// binds.ts — what a widget's bind names, read per widget on every render: the project setting a bind stands for.
// The prefix is Rust's (the generated constants), so the build that writes the bind and the card that reads it agree.

import { PROJECT_BIND } from "@studio/k7-wasm/consts";

/** The project setting a bind names, or null for any other bind. */
export const projectBindField = (bind: string | undefined): string | null => {
  if (!bind?.startsWith(PROJECT_BIND)) return null;
  const field = bind.slice(PROJECT_BIND.length);
  return field.length ? field : null;
};
