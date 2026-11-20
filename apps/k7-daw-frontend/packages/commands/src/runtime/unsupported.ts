// unsupported.ts — what a host member throws when its server route is not there yet: the runtime turns it into a refusal.
// What a reader gets wrong: it is a known gap, not a failure, so the run refuses (nothing committed) and names the member.

export class Unsupported extends Error {
  constructor(readonly member: string) {
    super(`needs ${member}`);
    this.name = "Unsupported";
  }
}

/** The refusal a page shows for a verb whose member is not built yet. */
export const unsupportedLine = (line: string, member: string): string =>
  `${line} is not available yet: it needs ${member}`;
