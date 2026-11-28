// FrozenChainsPage.tsx — the Library page's second section: the account's frozen chains, kept in its own
// library (`@account/chains/`), and the libraries' cold chains beside them, read-only, to copy from. A chain is
// built and edited on the BENCH, with the engine's panels and sound, and saved back by `chain save`.
// See ../../docs/frozenChains.md.

import { askConfirm, askText } from "@studio/project-shell/ask";
import { launchUrl } from "@studio/project-shell/launch";
import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import { EmptyState } from "@studio/ui/empty-state";
import { ErrorCard } from "@studio/ui/error-card";
import { Copy, Pencil, Plus, Radio, Snowflake, Trash2 } from "@studio/ui/icons";
import { RowGroup } from "@studio/ui/row-group";
import { SectionHeading } from "@studio/ui/section-heading";
import { Spinner } from "@studio/ui/spinner";
import type { FrozenChain } from "@studio/web-protocol";
import { BENCH_REPO, benchProject } from "@studio/web-protocol/projects";
import { byPeoplePath } from "@studio/web-protocol/tree";
import { useCallback, useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { useDesktopEngine } from "../settings/desktopEngine";
import { engineSlots, ownedEngines } from "../settings/engineSlots";
import type { PublicEngine } from "../types";
import { type AccountFiles, accountFiles } from "./api";
import { SlotButton, type SlotTarget } from "./SlotButton";

/** A chain's name as the file service takes it: one path segment. */
const FROZEN_NAME = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/;

/** Open the bench on `ref`, or on the bare rig: where a chain is built and edited. */
const openBench = (ref: string | null): void =>
  window.location.assign(
    launchUrl(window.location.origin, BENCH_REPO, benchProject(ref), "k7"),
  );

/** A copy's name: the source's last segment, numbered past the account's own names. */
function copyName(ref: string, taken: Set<string>): string {
  const base = (ref.split("/").pop() ?? "chain").replace(
    /[^A-Za-z0-9_-]+/g,
    "-",
  );
  let name = base;
  for (let i = 2; taken.has(name); i++) name = `${base}-${i}`;
  return name;
}

function Row({
  chain,
  onEdit,
  onCopy,
  onDelete,
  slot,
  here,
  remote,
  onSlot,
}: {
  chain: FrozenChain;
  onEdit?: () => void;
  onCopy: () => void;
  onDelete?: () => void;
  /** Its slot on this machine: none, kept, or the enabled one (../../docs/slots.md). */
  slot?: "kept" | "enabled" | null;
  /** This machine runs an engine, and the engines owned elsewhere: where a slot may go. */
  here?: boolean;
  remote?: PublicEngine[];
  /** Absent on the libraries' chains, which are copied first. */
  onSlot?: (t: SlotTarget) => void;
}) {
  const input = chain.input
    ? m["frozen.input"]({
        channel: Array.isArray(chain.input.channel)
          ? chain.input.channel.join("+")
          : chain.input.channel,
      })
    : null;
  const summary = chain.unreadable
    ? chain.unreadable
    : [
        ...(input ? [input] : []),
        ...chain.steps.map(
          (s) =>
            `${s.module.split("/").pop()}${s.bypass ? m["frozen.bypass"]() : ""}`,
        ),
      ].join(" → ");
  // The bench plays it live, so a chain that cannot run live is edited nowhere: the reason says why.
  const editTitle = chain.liveRefusal
    ? m["frozen.notLive"]({ why: chain.liveRefusal })
    : m["frozen.editOnBench"]();
  return (
    <div className="flex min-h-16 items-center gap-3 px-3.5 py-2.5">
      <span
        aria-hidden
        className="grid size-10 shrink-0 place-items-center rounded-xl bg-(--k7-glyph-tile) text-primary-text [&_svg]:size-5"
      >
        {chain.cold ? <Snowflake /> : <Radio />}
      </span>
      <span className="min-w-0 flex-1">
        <Chip variant="anchor" className="max-w-full truncate">
          {chain.ref}
        </Chip>
        <span
          className={`type-secondary mt-1 block truncate ${chain.unreadable ? "text-error" : "text-fg-2"}`}
          title={summary}
        >
          {summary}
        </span>
        <span className="type-status flex items-center gap-2 text-fg-3">
          {chain.liveRefusal
            ? m["frozen.notLive"]({ why: chain.liveRefusal })
            : m["frozen.live"]()}
          {slot && (
            <Chip variant="status" tone="renamed">
              {slot === "enabled"
                ? m["frozen.slotEnabled"]()
                : m["frozen.slotKept"]()}
            </Chip>
          )}
        </span>
      </span>
      {onEdit && (
        <Button
          size="icon"
          variant="ghost"
          disabled={!!chain.liveRefusal}
          onClick={onEdit}
          aria-label={editTitle}
          title={editTitle}
        >
          <Pencil className="size-5" />
        </Button>
      )}
      <Button
        size="icon"
        variant="ghost"
        onClick={onCopy}
        aria-label={m["frozen.duplicateNamed"]({ name: chain.name })}
        title={m["frozen.duplicate"]()}
      >
        <Copy className="size-5" />
      </Button>
      {onSlot && (
        <SlotButton
          here={!!here}
          remote={remote ?? []}
          refusal={chain.liveRefusal}
          onSave={onSlot}
        />
      )}
      {onDelete && (
        <Button
          size="icon"
          variant="ghost"
          onClick={onDelete}
          aria-label={m["frozen.deleteNamed"]({ name: chain.name })}
          title={m["frozen.delete"]()}
        >
          <Trash2 className="size-5" />
        </Button>
      )}
    </div>
  );
}

/** The section under the libraries; `files` is the account's file service, `null` when none answers. */
export function FrozenChainsSection({ files }: { files: AccountFiles | null }) {
  const [chains, setChains] = useState<FrozenChain[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  // This machine's engine, whose slots a chain is saved into; null where none runs here (../../docs/slots.md).
  const engine = useDesktopEngine();
  const slotOf = (c: FrozenChain): "kept" | "enabled" | null => {
    const s = engine?.snapshot?.slots?.find((x) => x.chain === c.ref);
    if (!s) return null;
    return engine?.snapshot?.enabledSlot === s.id ? "enabled" : "kept";
  };
  // The engines this account owns elsewhere, whose slots their server sets (../../docs/slots.md).
  const [remote, setRemote] = useState<PublicEngine[]>([]);
  const hereId = engine?.snapshot?.engineId;
  useEffect(() => {
    let live = true;
    void ownedEngines(hereId).then((list) => live && setRemote(list));
    return () => {
      live = false;
    };
  }, [hereId]);
  const saveSlot = async (c: FrozenChain, t: SlotTarget) => {
    if (!files) return;
    setError(null);
    try {
      if (t.kind === "here") {
        await accountFiles.saveSlot(files, c.ref);
        return;
      }
      await engineSlots.save(t.engine, c.ref);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const reload = useCallback(async (f: AccountFiles) => {
    try {
      const listed = await accountFiles.frozenChains(f);
      // Mine and the cold ones are split on render; each reads for people (../../docs/listOrder.md).
      setChains([...listed.chains].sort((a, b) => byPeoplePath(a.ref, b.ref)));
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);

  useEffect(() => {
    if (files) void reload(files);
  }, [files, reload]);

  const mine = (chains ?? []).filter((c) => c.mine);
  const cold = (chains ?? []).filter((c) => !c.mine);
  const taken = new Set(mine.map((c) => c.name));

  const remove = async (c: FrozenChain) => {
    if (!files) return;
    const ok = await askConfirm({
      title: m["frozen.deleteTitle"]({ name: c.name }),
      body: m["frozen.deleteBody"](),
      confirmLabel: m["frozen.delete"](),
      danger: true,
    });
    if (!ok) return;
    try {
      await accountFiles.deleteFrozenChain(files, c.name);
      await reload(files);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const duplicate = async (c: FrozenChain) => {
    if (!files) return;
    const name = (
      await askText({
        title: m["frozen.duplicate"](),
        body: m["frozen.duplicateBody"]({ ref: c.ref }),
        initial: copyName(c.name, taken),
        placeholder: m["frozen.namePlaceholder"](),
        confirmLabel: m["frozen.duplicate"](),
        // A bad name is said in the dialog, which stays open with what was typed.
        validate: (v) =>
          FROZEN_NAME.test(v.trim()) ? "" : m["frozen.badName"](),
      })
    )?.trim();
    if (!name) return;
    setError(null);
    try {
      await accountFiles.copyFrozenChain(files, c.ref, name);
      await reload(files);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <section
      id="frozen-chains"
      aria-labelledby="frozen-chains-title"
      className="flex scroll-mt-6 flex-col gap-4"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex flex-col gap-1">
          <h2 id="frozen-chains-title" className="type-headline">
            {m["frozen.title"]()}
          </h2>
          {chains && (
            <p className="type-secondary text-fg-2 tabular-nums">
              {m["frozen.count"]({ mine: mine.length, cold: cold.length })}
            </p>
          )}
        </div>
        {files && (
          <Button onClick={() => openBench(null)}>
            <Plus className="size-5" /> {m["frozen.new"]()}
          </Button>
        )}
      </div>
      {files && chains === null && !error ? (
        <div className="type-secondary flex items-center gap-2 text-fg-2">
          <Spinner /> {m["libs.reading"]()}
        </div>
      ) : files === null ? (
        <ErrorCard title={m["frozen.noServer"]()} />
      ) : (
        <div className="flex flex-col gap-6">
          <p className="type-secondary text-fg-2">{m["frozen.summary"]()}</p>
          {error && <ErrorCard title={error} />}
          <section className="flex flex-col gap-2">
            <SectionHeading className="px-1">
              {m["frozen.mine"]()}
            </SectionHeading>
            {mine.length === 0 ? (
              <EmptyState
                glyph={<Snowflake />}
                title={m["frozen.mineEmptyTitle"]()}
              >
                {m["frozen.mineEmpty"]()}
              </EmptyState>
            ) : (
              <RowGroup>
                {mine.map((c) => (
                  <Row
                    key={c.ref}
                    chain={c}
                    onEdit={() => openBench(c.ref)}
                    onCopy={() => void duplicate(c)}
                    onDelete={() => void remove(c)}
                    slot={slotOf(c)}
                    here={!!engine}
                    remote={remote}
                    onSlot={(t) => void saveSlot(c, t)}
                  />
                ))}
              </RowGroup>
            )}
            <p className="type-status px-1 text-fg-3">
              {m["frozen.howToUse"]()}{" "}
              <code className="type-mono-value">
                clip process &lt;clip&gt; @account/&lt;name&gt;
              </code>
            </p>
          </section>
          <section className="flex flex-col gap-2">
            <SectionHeading className="px-1">
              {m["frozen.cold"]()}
            </SectionHeading>
            {cold.length === 0 ? (
              <p className="type-secondary px-1 text-fg-2">
                {m["frozen.coldEmpty"]()}
              </p>
            ) : (
              <RowGroup>
                {cold.map((c) => (
                  <Row key={c.ref} chain={c} onCopy={() => void duplicate(c)} />
                ))}
              </RowGroup>
            )}
            {cold.length > 0 && (
              <p className="type-status px-1 text-fg-3">
                {m["frozen.coldNote"]()}
              </p>
            )}
          </section>
        </div>
      )}
    </section>
  );
}
