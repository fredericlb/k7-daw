// serverClock.ts — the capture zone's clock as the SERVER keeps it: a capture is filed under the day and HH-MM-SS of
// the server's own zone, which may not be the runner's (a staging server on UTC, a runner in Paris).
// No route reports a zone, so the offset is learnt from a path the server filed a known instant under (e2e/docs/conformance.md).
import { expect } from "@playwright/test";

const pad = (n: number) => String(n).padStart(2, "0");

/** Months no zone changed its clock in during the 1990s: a Windows clock applies today's rules to them correctly. */
const STEADY_MONTHS = [0, 5, 6, 11];

/** A day of the 1990s no person recorded on, and its instant at 10:05:07 UTC: the same day on any clock from UTC-10 to UTC+13:54. */
export function oldDay(): { day: string; at: Date } {
  const at = new Date(
    Date.UTC(
      1990 + Math.floor(Math.random() * 10),
      STEADY_MONTHS[Math.floor(Math.random() * STEADY_MONTHS.length)],
      1 + Math.floor(Math.random() * 28),
      10,
      5,
      7,
    ),
  );
  return { day: at.toISOString().slice(0, 10), at };
}

/** Minutes the server's clock is ahead of UTC at `at`, from a capture path it filed `at` under; a whole zone or it fails. */
export function offsetOf(at: Date, path: string): number {
  const m =
    /recordings\/(\d{4}-\d\d-\d\d)\/(?:moment-)?(\d\d)-(\d\d)-(\d\d)/.exec(
      path,
    );
  expect(m, `${path}: no capture day and stamp`).not.toBeNull();
  const [, day, h, min, s] = m as RegExpExecArray;
  const wall = Date.parse(`${day}T${h}:${min}:${s}Z`);
  const minutes = (wall - Math.floor(at.getTime() / 1000) * 1000) / 60_000;
  const zone = Number.isInteger(minutes) && minutes % 15 === 0;
  expect(
    zone && Math.abs(minutes) <= 14 * 60,
    `${path} for ${at.toISOString()}: ${minutes} min is no zone`,
  ).toBe(true);
  return minutes;
}

/** The capture day and HH-MM-SS stamp of `at` on a clock `offset` minutes ahead of UTC. */
export function onServer(
  at: Date,
  offset: number,
): { day: string; stamp: string } {
  const d = new Date(Math.floor(at.getTime() / 1000) * 1000 + offset * 60_000);
  return {
    day: `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}`,
    stamp: `${pad(d.getUTCHours())}-${pad(d.getUTCMinutes())}-${pad(d.getUTCSeconds())}`,
  };
}
