// boards.mjs — writes one static "Interactions · <screen>" board (Ix<nn>.dc.html) per drawn screen,
// plus entries.json (each board's place on the canvas), from the Sheet's CSV export.
// Publishing them to the design canvas is a manual Artifact step: apps/k7/frontend/docs/interactionsSheet.md.
// Usage: node scripts/interactions/boards.mjs <out dir> [csv dir]

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { CSV_DIR, loadSheet } from "./sheet.mjs";

const outDir = process.argv[2];
if (!outDir) throw new Error("usage: boards.mjs <out dir> [csv dir]");
const { screens } = loadSheet(process.argv[3] ?? CSV_DIR);

const [W, H, X, PAD, GAP, HEAD] = [1600, 900, 3250, 28, 28, 92];
const MONO = "'JetBrains Mono', ui-monospace, Menlo, monospace";
const ESC = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#x27;",
};
const E = (s) => s.replace(/[&<>"']/g, (c) => ESC[c]);
const code = (t) =>
  E(t)
    .split("`")
    .map((x, i) =>
      i % 2 ? `<span style="font-family: ${MONO}">${x}</span>` : x,
    )
    .join("");
const plain = (t) => t.replaceAll("`", "");
// Python's round(v, 1) text, which the published boards carry: 13.0, not 13.
const pyRound1 = (v) => {
  const r = Number(v.toFixed(1));
  return Number.isInteger(r) ? `${r}.0` : `${r}`;
};
// Column fractions of the text area: interaction, mouse, keyboard, touch.
const FR = [0.24, 0.27, 0.25, 0.24];
const len = (t) => [...plain(t)].length;
const nlines = (t, f, w) =>
  Math.max(1, Math.ceil((len(t) * 0.6 * f) / Math.max(w, 1)));

function geom(cols, f) {
  const cw = (W - 2 * PAD - (cols - 1) * GAP) / cols;
  const idw = Math.round(f * 0.62 * 5 + 8);
  const chw = 66;
  const tw = cw - idw - chw - 4 * 8;
  return { idw, chw, ws: FR.map((x) => tw * x - 4) };
}
const cells = (r) => [
  r.name + (r.nd ? " † not drawn" : ""),
  r.mouse,
  r.key,
  r.touch,
];

function split(hs, cols, avail) {
  const sum = (a) => a.reduce((n, h) => n + h, 0);
  if (cols === 1) return sum(hs) <= avail ? [hs.length] : null;
  const tot = sum(hs);
  let acc = 0;
  let i = 0;
  for (i = 0; i < hs.length; i++) {
    if (acc + hs[i] > tot / 2) break;
    acc += hs[i];
  }
  if (i === hs.length) i = hs.length - 1;
  for (const k of [i, i + 1, i - 1])
    if (
      k > 0 &&
      k < hs.length &&
      sum(hs.slice(0, k)) <= avail &&
      sum(hs.slice(k)) <= avail
    )
      return [k, hs.length - k];
  return null;
}

function layout(rows, avail) {
  for (const L of [null, 3, 2, 1])
    for (const cols of L === null ? [1, 2] : [2])
      for (const f of L === null ? [13, 12, 11, 10] : [10, 9.5, 9]) {
        const { ws } = geom(cols, f);
        const lh = f * 1.3;
        const hs = rows.map(
          (r) =>
            Math.max(
              ...cells(r).map((t, j) => Math.min(L ?? 99, nlines(t, f, ws[j]))),
            ) *
              lh +
            (L === null ? 8 : 4),
        );
        const sp = split(hs, cols, avail - (f + 14));
        if (sp) return { cols, f, L, split: sp };
      }
  throw new Error(`does not fit: ${rows.length}`);
}

const clamp = (L) =>
  L === null
    ? ""
    : `; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: ${L}; overflow: hidden`;
const LABEL = { T: "iPad", M: "iPhone", D: "Desktop" };

function chips(r, f) {
  return ["T", "M", "D"]
    .map((d) => {
      const on = r.devices.includes(d);
      const st = on
        ? "background: rgba(111,79,224,0.32); color: #f6f3fb"
        : "color: #6e6680; border: 1px dashed rgba(255,255,255,0.14)";
      return `<span aria-label="${LABEL[d]}${on ? "" : " (no)"}" style="width: 18px; height: ${Math.round(f * 1.3)}px; border-radius: 5px; box-sizing: border-box; display: inline-flex; align-items: center; justify-content: center; font-family: ${MONO}; font-size: ${Math.min(f, 11)}px; font-weight: 600; ${st}">${on ? d : "·"}</span>`;
    })
    .join("");
}

function column(rows, lay) {
  const { f, L } = lay;
  const { idw, chw } = geom(lay.cols, f);
  const lh = pyRound1(f * 1.3);
  const tmpl = `${idw}px ${FR.map((x) => `${x}fr`).join(" ")} ${chw}px`;
  const hd = [
    "ID",
    "INTERACTION",
    "MOUSE / TRACKPAD",
    "KEYBOARD",
    "TOUCH",
    "T M D",
  ]
    .map((x) => `<span>${x}</span>`)
    .join("");
  const out = [
    `<div style="display: grid; grid-template-columns: ${tmpl}; gap: 8px; padding-bottom: 6px; border-bottom: 1px solid rgba(255,255,255,0.12); font-size: 11px; line-height: ${f}px; font-weight: 700; letter-spacing: 0.6px; color: #aaa2ba">${hd}</div>`,
  ];
  for (const r of rows) {
    const tip = E(
      `${r.id} ${plain(r.name)} — Mouse: ${plain(r.mouse)} · Keyboard: ${plain(r.key)} · Touch: ${plain(r.touch)}`,
    );
    const nd = r.nd
      ? '<span style="margin-left: 6px; font-weight: 400; font-size: 0.85em; color: #aaa2ba">† not drawn</span>'
      : "";
    const c = (t, extra = "") =>
      `<span style="min-width: 0${clamp(L)}${extra}">${code(t)}</span>`;
    const g = (t) => (t.startsWith("—") ? c(t, "; color: #6e6680") : c(t));
    out.push(
      `<div title="${tip}" style="display: grid; grid-template-columns: ${tmpl}; gap: 8px; align-items: start; font-size: ${f}px; line-height: ${lh}px; color: #c5bdd4">` +
        `<span style="font-family: ${MONO}; font-size: ${Math.min(f, 12)}px; color: #c9b6ff">${r.id}</span>` +
        `<span style="min-width: 0; font-weight: 700; color: #f6f3fb${clamp(L)}">${code(r.name)}${nd}</span>` +
        `${g(r.mouse)}${g(r.key)}${g(r.touch)}<span style="display: flex; gap: 3px">${chips(r, f)}</span></div>`,
    );
  }
  return `<div style="min-width: 0; display: flex; flex-direction: column; gap: ${L === null ? 6 : 3}px">${out.join("")}</div>`;
}

function body(sc) {
  const rows = sc.rows;
  if (!rows.length) {
    const name = (c) => screens.find((s) => s.code === c).name;
    const where = sc.shared.map((c) => `${c} ${E(name(c))}`).join(" and ");
    return (
      '<div style="flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; border: 1px dashed rgba(255,255,255,0.16); border-radius: 24px; text-align: center; padding: 32px">' +
      '<div style="font-size: 20px; font-weight: 700">No interaction of its own</div>' +
      `<p style="margin: 0; max-width: 620px; font-size: 15px; line-height: 21px; color: #c5bdd4">Every control drawn on this screen is listed where it first appears: <span style="font-family: ${MONO}; color: #c9b6ff">${where}</span>.</p></div>`
    );
  }
  const lay = layout(rows, H - 2 * PAD - HEAD);
  const [first] = lay.split;
  const parts =
    lay.split.length > 1 ? [rows.slice(0, first), rows.slice(first)] : [rows];
  const cols = parts.map((part) => column(part, lay));
  return `<div style="flex: 1; min-height: 0; overflow: hidden; display: grid; grid-template-columns: repeat(${lay.cols}, minmax(0, 1fr)); gap: ${GAP}px; align-content: start">${cols.join("")}</div>`;
}

function board(sc) {
  const rows = sc.rows;
  const cnt = (d) => rows.filter((r) => r.devices.includes(d)).length;
  const c = sc.code;
  const sub = rows.length
    ? `${rows.length} interaction${rows.length !== 1 ? "s" : ""} · iPad ${c}T ${cnt("T")} · iPhone ${c}M ${cnt("M")} · Desktop ${c}D ${cnt("D")} · one id for every device · full text in INTERACTIONS.md`
    : "No interaction of its own";
  const head =
    `<header style="display: flex; flex-direction: column; gap: 4px; height: ${HEAD - 16}px; flex-shrink: 0">` +
    `<div style="font-family: ${MONO}; font-size: 12px; letter-spacing: 1px; color: #c9b6ff">${c} · INTERACTIONS · ${c}T ${c}M ${c}D</div>` +
    `<h1 style="margin: 0; font-size: 20px; line-height: 26px; font-weight: 700">${E(sc.name)}</h1>` +
    `<div style="font-size: 13px; line-height: 18px; color: #c5bdd4">${sub}</div></header>`;
  const title = `Interactions · ${sc.name}`;
  return { title, html: page(title, head, body(sc)) };
}

const page = (title, head, body) => `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${E(title)}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link href="https://fonts.googleapis.com/css2?family=Sansation:wght@300;400;700&amp;family=JetBrains+Mono:wght@400;600&amp;display=swap" rel="stylesheet">
<style>
body{margin:0}
</style>
</helmet>
<div style="width: ${W}px; height: ${H}px; box-sizing: border-box; padding: ${PAD}px; display: flex; flex-direction: column; gap: 16px; overflow: hidden; background: #07040f; color: #f6f3fb; font-family: 'Sansation', system-ui, sans-serif; -webkit-font-smoothing: antialiased">
${head}
${body}
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{"$preview":{"width":${W},"height":${H}}}'>
class Component extends DCLogic {
renderVals() {
return {};
}
}
</script>
</body>
</html>
`;

mkdirSync(outDir, { recursive: true });
const entries = {};
for (const sc of screens.filter((s) => s.drawn)) {
  const name = `Ix${sc.code}.dc.html`;
  const { title, html } = board(sc);
  writeFileSync(join(outDir, name), html);
  entries[name] = { x: X, y: sc.y, w: W, h: H, title };
}
writeFileSync(join(outDir, "entries.json"), JSON.stringify(entries, null, 1));
console.log(
  `${outDir}: ${Object.keys(entries).length} boards and entries.json`,
);
