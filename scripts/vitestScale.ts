// vitestScale.ts — the load factor for test timeouts, read from K7_TEST_TIMEOUT_SCALE (default 1).
// What a reader gets wrong: it only widens a bound; the gate scripts set it, a developer's shell never does.

const parsed = Number(process.env.K7_TEST_TIMEOUT_SCALE);
export const timeoutScale = Number.isFinite(parsed) && parsed >= 1 ? parsed : 1;

export const scaled = (ms: number): number => Math.round(ms * timeoutScale);
