// logProblems.ts — which Logs lines are PROBLEMS: failures of the app and engine lines that read as one, never a refusal.
// A verb that declines (`refused`) is a command's answer and is listed under Commands; the header counts what this decides.

import type { LogLine } from "./components/LogsPanel";

// The engine machine writes plain text with no level, so a problem is recognised by its words.
const ENGINE_PROBLEM = /\b(error|failed|failure|panic|fatal|warning|warn)\b/i;

/** A failure of the app, or an engine or file line that says it failed. */
export const isProblemLine = (l: LogLine): boolean =>
  l.kind === "error" ||
  ((l.kind === "engine" || l.kind === "file") && ENGINE_PROBLEM.test(l.text));

/** The lines the Commands tab lists: what was run and what a verb refused. */
export const isCommandLine = (l: LogLine): boolean =>
  l.kind === "cmd" || l.kind === "refused";
