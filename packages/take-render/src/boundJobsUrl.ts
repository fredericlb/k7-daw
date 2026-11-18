// boundJobsUrl.ts — the jobs path of the engine machine this account is bound to, as the hub last said.
// A module of its own so a render can read it without the file service; docs/rendering.md.

let jobsUrl: string | null = null;

/** The bound machine's jobs path, or null while no engine is bound. */
export function boundJobsUrl(): string | null {
  return jobsUrl;
}

/** Set by the hub's binding route, and only there. */
export function setJobsUrl(url: string | null): void {
  jobsUrl = url;
}
