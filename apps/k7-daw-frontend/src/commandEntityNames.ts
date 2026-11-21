// commandEntityNames.ts — what the command editor's filter chips call a command's head, in the app's language.
// A head the catalogue grows and no message names yet shows as written: a chip is never blank.

import { m } from "./paraglide/messages.js";

const NAMES: Record<string, () => string> = {
  node: m["cmds.entity.node"],
  picture: m["cmds.entity.picture"],
  event: m["cmds.entity.event"],
  sequence: m["cmds.entity.sequence"],
  track: m["cmds.entity.track"],
  project: m["cmds.entity.project"],
  system: m["cmds.entity.system"],
  transport: m["cmds.entity.transport"],
  timeline: m["cmds.entity.timeline"],
  graph: m["cmds.entity.graph"],
  clip: m["cmds.entity.clip"],
  playlist: m["cmds.entity.playlist"],
  module: m["cmds.entity.module"],
  curve: m["cmds.entity.curve"],
  task: m["cmds.entity.task"],
  assembly: m["cmds.entity.assembly"],
  selection: m["cmds.entity.selection"],
  library: m["cmds.entity.library"],
  chain: m["cmds.entity.chain"],
  ui: m["cmds.entity.ui"],
};

/** The chip's text for a command head. */
export const entityName = (head: string): string => NAMES[head]?.() ?? head;
