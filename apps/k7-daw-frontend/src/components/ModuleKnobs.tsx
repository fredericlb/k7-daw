// ModuleKnobs.tsx — the first three knobs of a module's panel, on its chain-dock card: the same plugin params
// the full row binds, read live off the params store. A summary, not the panel: the rest stays on "Controls".

import {
  type PluginParamsStore,
  usePluginParamValue,
} from "@studio/engine-browser/params";
import type { WebWidget } from "@studio/graph-model";
import { Knob } from "@studio/ui/knob";
import type { GraphNode } from "@studio/ui-chains/graph";

const SHOWN = 3;

/** The panel's knob and slider widgets that name a param and a range, in grid order — the card's candidates. */
function knobWidgets(n: GraphNode): WebWidget[] {
  const all = [...Object.values(n.ui?.widgets ?? {})];
  for (const t of n.ui?.tabs ?? []) all.push(...Object.values(t.widgets));
  return all
    .filter(
      (w) =>
        (w.type === "knob" || w.type === "slider") &&
        !!w.bind &&
        w.min !== undefined &&
        w.max !== undefined,
    )
    .sort((a, b) => a.row - b.row || a.col - b.col)
    .slice(0, SHOWN);
}

function ParamKnob({
  node,
  widget,
  store,
  onSet,
}: {
  node: GraphNode;
  widget: WebWidget;
  store: PluginParamsStore;
  onSet: (id: string, name: string, value: number) => void;
}) {
  const bind = widget.bind ?? "";
  const live = usePluginParamValue(store, node.pluginId ?? null, bind);
  const file = node.params?.[bind];
  const value =
    live ??
    (typeof file === "number" ? file : (node.defaults?.[bind] ?? widget.min));
  return (
    <div className="flex min-w-0 flex-1 flex-col items-center gap-0.5">
      <Knob
        label={widget.label ?? bind}
        size={36}
        value={value ?? 0}
        min={widget.min}
        max={widget.max}
        step={widget.step}
        format={(v) => (Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(2))}
        onValueChange={(v) => node.pluginId && onSet(node.pluginId, bind, v)}
      />
      <span className="type-secondary w-full truncate text-center text-fg-2 uppercase">
        {widget.label ?? bind}
      </span>
    </div>
  );
}

/** Up to three knobs of the card's module; nothing when it has no plugin to address or no knob panel. */
export function ModuleKnobs({
  node,
  store,
  onSet,
}: {
  node: GraphNode;
  store?: PluginParamsStore;
  onSet?: (id: string, name: string, value: number) => void;
}) {
  if (!store || !onSet || !node.pluginId) return null;
  const ws = knobWidgets(node);
  if (!ws.length) return null;
  return (
    <div className="flex items-start gap-1">
      {ws.map((w) => (
        <ParamKnob
          key={w.bind}
          node={node}
          widget={w}
          store={store}
          onSet={onSet}
        />
      ))}
    </div>
  );
}
