// assemblyEdit.ts — one assembly-template writer run on its text, with the writer's refusal handed back as a sentence.
// What a reader would get wrong: a refusal is NOT swallowed — the screen shows `error`, so a bad file never edits silently.

import {
  type AssemblyOp,
  editAssembly,
  readAssemblyTemplate,
} from "@studio/k7-wasm/text";

export interface AssemblyEdit {
  text: string;
  /** Why the writer refused (the text is then unchanged), or null. */
  error: string | null;
}

/** The first thing wrong with a template's text, as a line a person reads; null when it reads. */
export function assemblyProblem(text: string): string | null {
  const read = readAssemblyTemplate(text);
  if (read.params) return null;
  const first = read.issues[0];
  if (!first) return "assembly.k7x does not read";
  return first.line ? `line ${first.line}: ${first.message}` : first.message;
}

/** The template's text with one writer run on it; unchanged, with the refusal, when the writer refuses. */
export function editedAssembly(text: string, op: AssemblyOp): AssemblyEdit {
  try {
    return { text: editAssembly(text, [op]).text, error: null };
  } catch (e) {
    return { text, error: e instanceof Error ? e.message : String(e) };
  }
}
