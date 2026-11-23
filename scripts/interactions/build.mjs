// build.mjs — writes apps/k7-daw-frontend/docs/INTERACTIONS.md from the Sheet's CSV export.
// The md is an output: a change goes to the Sheet, then here (apps/k7-daw-frontend/docs/interactionsSheet.md).
// Usage: node scripts/interactions/build.mjs [csv dir] [out file]

import { writeFileSync } from "node:fs";
import { CSV_DIR, loadSheet, SHEET_URL } from "./sheet.mjs";

const dir = process.argv[2] ?? CSV_DIR;
const outFile = process.argv[3] ?? "apps/k7-daw-frontend/docs/INTERACTIONS.md";
const { screens, areas, gaps } = loadSheet(dir);

const HEAD = [
  `<!-- Generated from the Sheet ${SHEET_URL} by scripts/interactions/build.mjs: do not edit by hand. -->`,
  "# K7 DAW interactions",
  "",
  "This inventory was read from main's source code on 2026-11-20. Nothing in it was tested on a device.",
  "",
  'Each interaction is listed once, under the first screen below that has it. "—" means that input has no way to do it.',
  "On touch, a long-press (500 ms, held still) acts as a right-click wherever a right-click menu exists.",
  "⌘/Ctrl means Cmd on macOS and Ctrl elsewhere.",
  "",
  "## Identifiers",
  "",
  "A screen is a row of the design canvas (an iPad, an iPhone and a Desktop board), numbered top to bottom; each device variant of a screen has its own identifier: `T` iPad (tablet), `M` iPhone (mobile), `D` Desktop. A screen with no board is listed here as not drawn.",
  "An interaction is `<screen number>.<nn>` (`03.05`), one id shared by every device so the three variants converge on the same behaviour. The Devices column says where it applies: `D` when a mouse, trackpad or keyboard can do it, `T` when touch or an iPad keyboard can, `M` when touch can. A row lives on the first screen that shows its element; global, header, transport, command bar and context-menu rows are not repeated on later screens.",
  `† marks a row whose element is not drawn on that screen's boards (it is kept on the screen of its area). The counts in the table are the rows each device can do.`,
  "",
  "| Screen | Tablet | Mobile | Desktop | Canvas board(s) |",
  "|---|---|---|---|---|",
];
const ROW_HEAD = [
  "| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |",
  "|---|---|---|---|---|---|",
];

const out = [...HEAD];
const count = (s, d) =>
  `${s.code}${d} (${s.rows.filter((r) => r.devices.includes(d)).length})`;
for (const s of screens) {
  const boards = s.drawn ? s.boards : "not drawn";
  out.push(
    `| ${s.code} ${s.name} | ${count(s, "T")} | ${count(s, "M")} | ${count(s, "D")} | ${boards} |`,
  );
}
out.push("");

const seen = new Set();
for (const s of screens) {
  out.push(`## ${s.code} ${s.name}${s.drawn ? "" : " (not drawn)"}`, "");
  if (!s.rows.length) {
    out.push(
      `No interaction of its own: its controls are listed under ${s.shared.join(" and ")}.`,
      "",
    );
    continue;
  }
  // Areas keep the order of their first row, so a row added later joins its area's table.
  const groups = new Map();
  for (const r of s.rows)
    groups.set(r.area, [...(groups.get(r.area) ?? []), r]);
  for (const [area, rows] of groups) {
    out.push(`### ${area}`, "");
    const top = area.split(" · ")[0];
    if (!seen.has(top) && areas.get(top)) {
      seen.add(top);
      out.push(areas.get(top), "");
    }
    out.push(...ROW_HEAD);
    for (const r of rows)
      out.push(
        `| ${r.id}${r.nd ? "†" : ""} | ${r.name} | ${r.mouse} | ${r.key} | ${r.touch} | ${r.devices.join(" ")} |`,
      );
    out.push("");
  }
}
out.push("## Gaps found while listing", "", ...gaps.map((g) => `- ${g}`));
writeFileSync(outFile, `${out.join("\n")}\n`);
console.log(
  `${outFile}: ${screens.length} screens, ${screens.reduce((n, s) => n + s.rows.length, 0)} interactions, ${gaps.length} gaps`,
);
