// sheet.mjs — reads the Sheet's CSV export (interactions, screens, areas, gaps) into one model.
// The CSVs are a copy of the Sheet, never edited by hand: ../../apps/k7-daw-frontend/docs/interactionsSheet.md.

import { readFileSync } from "node:fs";
import { join } from "node:path";

export const SHEET_URL = "https://claude.ai/artifact/GWymnBJpibPCE4b32ZR1gz";
export const CSV_DIR = "apps/k7-daw-frontend/docs/interactions";

/** RFC 4180: quoted fields may hold commas, doubled quotes and line breaks. */
export function parseCsv(text) {
  const rows = [];
  let row = [];
  let cell = "";
  let quoted = false;
  const s = text.replace(/^﻿/, "");
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (quoted) {
      if (ch === '"' && s[i + 1] === '"') {
        cell += '"';
        i++;
      } else if (ch === '"') quoted = false;
      else cell += ch;
    } else if (ch === '"') quoted = true;
    else if (ch === ",") {
      row.push(cell);
      cell = "";
    } else if (ch === "\n" || ch === "\r") {
      if (ch === "\r" && s[i + 1] === "\n") i++;
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else cell += ch;
  }
  if (cell !== "" || row.length) rows.push([...row, cell]);
  return rows.filter((r) => r.some((c) => c.trim() !== ""));
}

/** Rows as objects keyed by the header, so a column the Sheet moves still reads. */
function table(dir, name, need) {
  const text = readFileSync(join(dir, `${name}.csv`), "utf8");
  const [head, ...rows] = parseCsv(text);
  for (const col of need)
    if (!head.includes(col)) throw new Error(`${name}.csv: no "${col}" column`);
  return rows.map((r) =>
    Object.fromEntries(head.map((h, i) => [h, (r[i] ?? "").trim()])),
  );
}

const yes = (v) => /^(true|yes|vrai|1|x)$/i.test(v);
const code = (v) => {
  if (!/^\d{1,2}$/.test(v)) throw new Error(`not a screen number: "${v}"`);
  return v.padStart(2, "0");
};

function screenOf(s) {
  const c = code(s.Number);
  const drawn = yes(s.Drawn);
  return {
    n: Number(c),
    code: c,
    name: s.Name,
    drawn,
    boards: drawn ? `${s["Canvas boards"]}, ${s["Interactions board"]}` : "",
    y: drawn ? Number(s["Canvas y"]) : null,
    shared: s["Shared with"] ? s["Shared with"].split(/\s+/).map(code) : [],
    rows: [],
  };
}

function rowOf(r) {
  if (!/^\d{2}\.\d{2}$/.test(r.ID)) throw new Error(`bad ID "${r.ID}"`);
  const devices = r.Devices.split(/\s+/).filter(Boolean);
  if (devices.some((d) => !["T", "M", "D"].includes(d)))
    throw new Error(`${r.ID}: devices "${r.Devices}"`);
  return {
    id: r.ID,
    area: r.Area,
    name: r.Interaction,
    mouse: r["Mouse / trackpad"],
    key: r.Keyboard,
    touch: r.Touch,
    devices,
    nd: yes(r["Not drawn"]),
  };
}

/** The whole Sheet: screens in number order, each with its rows in ID order. */
export function loadSheet(dir) {
  const IX = ["ID", "Screen", "Interaction", "Mouse / trackpad", "Keyboard"];
  const ix = table(dir, "interactions", [
    ...IX,
    "Touch",
    "Devices",
    "Not drawn",
    "Area",
  ]);
  const SC = ["Number", "Name", "Canvas boards", "Interactions board"];
  const sc = table(dir, "screens", [...SC, "Drawn", "Shared with", "Canvas y"]);
  const screens = sc.map(screenOf).sort((a, b) => a.n - b.n);
  const byCode = new Map(screens.map((s) => [s.code, s]));
  const ids = new Set();
  for (const r of ix) {
    if (ids.has(r.ID)) throw new Error(`ID ${r.ID} twice`);
    ids.add(r.ID);
    const s = byCode.get(code(r.Screen));
    if (!s) throw new Error(`${r.ID}: no screen ${r.Screen}`);
    s.rows.push(rowOf(r));
  }
  for (const s of screens) s.rows.sort((a, b) => a.id.localeCompare(b.id));
  const areas = new Map(
    table(dir, "areas", ["Area", "Intro"]).map((a) => [a.Area, a.Intro]),
  );
  const gaps = table(dir, "gaps", ["Gap"]).map((g) => g.Gap);
  return { screens, areas, gaps };
}
