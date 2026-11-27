// IconPicker.tsx — pick a lucide icon by name. The whole set (~1500), searched, in a popover.
//
// lucide-react is already this app's icon library, so nothing new is imported into the project to
// get here. What IS new is importing it as a NAMESPACE: everywhere else picks its icons by name and
// is tree-shaken down to those, while a picker has to know all of them. That costs the bundle the
// full set, which is why this file is only ever reached through React.lazy — the settings page pays
// it when it opens, and nobody else pays it at all.

import { SearchList } from "@studio/ui/search-list";
import { useMemo } from "react";
import { useIcons } from "../lucideIcons";
import { m } from "../paraglide/messages.js";

export default function IconPicker({
  value,
  onPick,
}: {
  value?: string;
  onPick: (name: string | undefined) => void;
}) {
  const set = useIcons();
  const icons = useMemo(() => Object.entries(set ?? {}), [set]);

  return (
    <div className="w-72">
      <SearchList
        items={icons}
        text={([name]) => name}
        onPick={([name]) => onPick(name)}
        isSelected={([name]) => name === value}
        itemLabel={([name]) => name}
        renderRow={([, Icon]) => <Icon className="size-4" />}
        emptyLabel={m["icons.noMatch"]()}
        pending={!set}
        searchLabel={m["icons.search"]()}
        // Two hundred rows is more than anyone scans, and fifteen hundred SVGs in a popover pause on every keystroke.
        limit={200}
        columns={8}
        autoFocus
        listClassName="grid max-h-56 grid-cols-8 gap-1"
        rowClassName="min-h-9 w-auto justify-center rounded-lg px-0 desk:min-h-8"
        // "None" first, so taking an icon back is as easy as choosing one.
        before={
          <button
            type="button"
            onClick={() => onPick(undefined)}
            className="type-secondary min-h-9 cursor-pointer rounded-lg px-3 text-left text-fg-2 outline-none hover:bg-on-fill focus-visible:bg-on-fill desk:min-h-8"
          >
            {m["icons.none"]()}
          </button>
        }
      />
    </div>
  );
}
