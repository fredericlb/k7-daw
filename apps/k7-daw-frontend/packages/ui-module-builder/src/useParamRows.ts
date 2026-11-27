// useParamRows.ts — every parameter a module's widgets may bind, unioned from all the sources that know some, each row with the most specific range known.
// The union is Rust's (`panel.param_rows`); what stays here is the page's own: the live hints it reads and the collator that orders the rows.
// See docs/builder.md.

import type { ParamMeta } from "@studio/engine-protocol";
import { paramRows, paramValues } from "@studio/k7-wasm/panel";
import type { Hints } from "@studio/web-protocol";
import { sortForPeople } from "@studio/web-protocol/tree";
import { useMemo } from "react";
import type { ModuleUI } from "./moduleUi";
import type { ParamRow } from "./ParamsPane";

export { stageFor } from "@studio/k7-wasm/panel";

/** `@base`, offered only where there ARE bases; the graph fills its entries — see docs/builder.md. */
function baseBinds(bases: string[]): { name: string; meta: ParamMeta }[] {
  if (!bases.length) return [];
  return [
    {
      name: "@base",
      meta: {
        min: 0,
        max: bases.length,
        states: [
          { label: "none", value: 0 },
          // Sorted for people, each keeping its place in the list as its value, as the graph's dropdown does.
          ...sortForPeople(
            bases.map((b, i) => ({ label: b, value: i + 1 })),
            (s) => s.label,
          ),
        ],
      },
    },
  ];
}

export function useParamRows({
  content,
  ui,
  io,
  path,
  hints,
  paramRef,
  pluginParamNames,
  pluginParamMeta,
  scanned,
  bases,
}: {
  content: string | null;
  ui: ModuleUI | null;
  io: string | null;
  path: string;
  hints: Hints | null;
  paramRef: string | null;
  pluginParamNames?: string[];
  pluginParamMeta?: Record<string, ParamMeta>;
  scanned: { names: string[]; meta: Record<string, ParamMeta> } | null;
  bases: string[];
}): { rows: ParamRow[]; values: Record<string, number> } {
  const params = paramRef ? (hints?.paramsByBase[paramRef] ?? []) : [];
  // Ranges for a built-in k7 defines; the engine's live report is better, so this only fills what it has not.
  const baseMeta = paramRef ? (hints?.paramMetaByBase[paramRef] ?? {}) : {};

  const rows = useMemo(
    () =>
      paramRows({
        content: content ?? undefined,
        bound: Object.values(ui?.widgets ?? {}).flatMap((w) =>
          w.bind ? [w.bind] : [],
        ),
        io: io ?? undefined,
        path,
        hintParams: params,
        hintMeta: baseMeta,
        pluginNames: pluginParamNames ?? [],
        pluginMeta: pluginParamMeta ?? {},
        scannedNames: scanned?.names ?? [],
        scannedMeta: scanned?.meta ?? {},
        baseBinds: baseBinds(bases),
      })
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((r) => ({ ...r, meta: r.meta as ParamMeta | undefined })),
    [
      content,
      io,
      ui,
      params,
      baseMeta,
      pluginParamNames,
      pluginParamMeta,
      scanned,
      bases,
      path,
    ],
  );

  // The same values as NUMBERS for the preview; anything that is not one falls back to the widget's min.
  const values = useMemo(
    () => (content == null ? {} : paramValues(content)),
    [content],
  );

  return { rows, values };
}
