// The one place a date or a clock is worded, for the médiathèque and the editor.
// The locale is always the APP's, passed in: the browser's own would print English dates in a French app.
// The styles and why there are only these: ../docs/recorder.md.

/** The named ways a moment in time is printed; a screen picks one, never its own options. */
export type DateStyle = "day" | "date" | "dateTime" | "time" | "timeSeconds";

const STYLES: Record<DateStyle, Intl.DateTimeFormatOptions> = {
  // A day as a page title: "Tuesday, October 27, 2026".
  day: { weekday: "long", day: "numeric", month: "long", year: "numeric" },
  // A day in a sentence: "Oct 27, 2026".
  date: { dateStyle: "medium" },
  // When something was made: "Oct 27, 2026, 1:13 PM".
  dateTime: { dateStyle: "medium", timeStyle: "short" },
  // A clock on a timeline: "1:13 PM".
  time: { hour: "numeric", minute: "2-digit" },
  timeSeconds: { hour: "numeric", minute: "2-digit", second: "2-digit" },
};

/** `at` in the named style and the app's language (a BCP 47 tag); an unreadable date prints empty. */
export function formatDate(
  at: Date | string | number,
  style: DateStyle,
  locale: string,
): string {
  const d = at instanceof Date ? at : new Date(at);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString(locale, STYLES[style]);
}
