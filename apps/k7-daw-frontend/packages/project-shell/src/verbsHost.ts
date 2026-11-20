// verbsHost.ts — where the command bar's verbs run: the page registers a host (a Worker), the file API's three verb calls go through it.
// What a reader gets wrong: nothing here knows of a Worker; a host is three functions, so the seam is tested over a fake.

import type {
  ChangedFile,
  CommandInfo,
  WebVideoProject,
} from "@studio/web-protocol";

/** One `/complete` candidate: the line it completes to, and what a list shows for its value instead. */
export interface VerbCompletion {
  line: string;
  label?: string;
}

/** What a verb run answers: the lines, and the views the caller asked to draw. */
export interface VerbRun {
  output: string[];
  refused: boolean;
  graph?: unknown;
  rev?: string;
  revBefore?: string;
  changed?: ChangedFile[];
  /** The host member a refusal is for, when the verb needs one that is not built yet. */
  unsupported?: string;
}

export interface VerbRunOptions {
  at?: string;
  want: string[];
  journal?: "join" | "skip";
}

/** The open project's picture: the document, or the compile's own refusal as a sentence. */
export type VideoAnswer = { doc: WebVideoProject } | { error: string };

export interface VerbsHost {
  catalogue(): Promise<CommandInfo[]>;
  complete(line: string): Promise<(string | VerbCompletion)[]>;
  run(line: string, options: VerbRunOptions): Promise<VerbRun>;
  videoDoc(): Promise<VideoAnswer>;
  videoErrors(): Promise<{ errors: string[]; diagnostics: never[] }>;
}

let host: VerbsHost | null = null;

/** The page's host; a test or a screen with no verbs passes null. */
export function setVerbsHost(h: VerbsHost | null): void {
  host = h;
}

export function verbsHost(): VerbsHost {
  if (!host) throw new Error("the verbs are not available on this page");
  return host;
}
