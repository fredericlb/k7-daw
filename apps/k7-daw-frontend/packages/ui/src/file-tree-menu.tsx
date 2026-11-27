// The FileTree's action menu: the list of offered actions, on the kit's PointMenu. An action that
// asks something (a name, a yes) hands it to the app's own question dialog through `TreeAsk`.
// The tree decides WHERE and FOR WHAT; see apps/k7-daw-frontend/packages/ui/docs/file-tree.md.

import { useMemo } from "react";
import type { TreeAction, TreeAsk, TreeNode, TreeTarget } from "./file-tree";
import { PointMenu, PointMenuItem } from "./point-menu";

export interface TreeMenuProps<N extends TreeNode> {
  target: TreeTarget<N>;
  x: number;
  y: number;
  actions: TreeAction<N>[];
  ask: TreeAsk;
  onClose: () => void;
}

export function TreeMenu<N extends TreeNode>({
  target,
  x,
  y,
  actions,
  ask,
  onClose,
}: TreeMenuProps<N>) {
  const offered = useMemo(
    () => actions.filter((a) => a.when?.(target) ?? true),
    [target, actions],
  );

  const label = (a: TreeAction<N>) =>
    typeof a.label === "function" ? a.label(target) : a.label;

  // The menu is gone by the time a question is up, so the answer runs from here, not from a row.
  const start = async (action: TreeAction<N>) => {
    const title = label(action);
    if (action.confirm) {
      const yes = await ask.confirm({
        title,
        body: action.confirm.message(target),
        confirmLabel: action.confirm.submit,
        danger: action.danger,
      });
      if (yes) action.run(target, "");
    } else if (action.prompt) {
      const { prompt } = action;
      const initial = prompt.initial?.(target) ?? "";
      const answer = await ask.text({
        title,
        body: prompt.hint?.(target, initial),
        initial,
        placeholder: prompt.placeholder,
        confirmLabel: prompt.submit,
      });
      if (answer?.trim()) action.run(target, answer.trim());
    } else action.run(target, "");
  };

  return (
    <PointMenu x={x} y={y} onClose={onClose} className="min-w-[11rem]">
      {offered.map((action) => (
        <PointMenuItem
          key={action.key}
          danger={action.danger}
          onSelect={() => void start(action)}
        >
          {label(action)}
        </PointMenuItem>
      ))}
    </PointMenu>
  );
}
