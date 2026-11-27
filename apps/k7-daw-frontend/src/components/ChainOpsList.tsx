// ChainOpsList.tsx — the body of a clip's "operations" menu: a search over every chain the project
// and the library offer, and the list to run one from. The clip panel and the editor's toolbar draw
// the same menu over the same clip (../clipOps.ts spells the command).

import {
  Popover,
  PopoverAnchor,
  PopoverContent,
  PopoverTrigger,
} from "@studio/ui/popover";
import { SearchList } from "@studio/ui/search-list";
import { type ReactNode, useState } from "react";
import { m } from "../paraglide/messages.js";

interface ListProps {
  search: string;
  onSearch: (q: string) => void;
  ops: string[];
  /** The chains are still being listed: a spinner rather than "none". */
  pending?: boolean;
  onPick: (chain: string) => void;
}

export function ChainOpsList({
  search,
  onSearch,
  ops,
  pending = false,
  onPick,
}: ListProps) {
  return (
    <SearchList
      items={ops}
      // The host filters with Rust's `filterOps` as the query changes, so the list only draws what it is given.
      rank={null}
      query={search}
      onQueryChange={onSearch}
      onPick={onPick}
      renderRow={(c) => <span className="truncate">{c}</span>}
      emptyLabel={m["clip.noChain"]()}
      pending={pending}
      searchLabel={m["clip.searchChain"]()}
      listClassName="max-h-64"
    />
  );
}

/** The list in a popover, closed by a pick: `trigger` opens it on press, `anchor` is for a host that opens it itself (`open`). */
export function ChainOpsPopover({
  trigger,
  anchor,
  open,
  onOpenChange,
  onPick,
  ...list
}: Omit<ListProps, "onPick"> & {
  trigger?: ReactNode;
  anchor?: ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  onPick: (chain: string) => void;
}) {
  const [own, setOwn] = useState(false);
  const set = onOpenChange ?? setOwn;
  return (
    <Popover open={open ?? own} onOpenChange={set}>
      {trigger ? (
        <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      ) : (
        <PopoverAnchor asChild>
          <span className="inline-flex">{anchor}</span>
        </PopoverAnchor>
      )}
      <PopoverContent align="start" className="w-64 p-1.5">
        <ChainOpsList
          {...list}
          onPick={(c) => {
            onPick(c);
            set(false);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}
