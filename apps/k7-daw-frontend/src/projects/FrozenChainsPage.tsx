// FrozenChainsPage.tsx — `/libraries/chains`: the account's frozen chains, kept in its own library
// (`@account/chains/`), and the libraries' cold chains beside them, read-only, to copy from. A chain is
// built and edited on the BENCH, with the engine's panels and sound, and saved back by `chain save`.
// See ../../docs/frozenChains.md.

import { askConfirm, askText } from "@studio/project-shell/ask";
import { launchUrl } from "@studio/project-shell/launch";
import { showToast, Toaster } from "@studio/project-shell/toast";
import { Button } from "@studio/ui/button";
import {
  Copy,
  Loader2,
  Pencil,
  Plus,
  Radio,
  Snowflake,
  Trash2,
} from "@studio/ui/icons";
import type { FrozenChain } from "@studio/web-protocol";
import { BENCH_REPO, benchProject } from "@studio/web-protocol/projects";
import { byPeoplePath } from "@studio/web-protocol/tree";
import { useCallback, useEffect, useState } from "react";
import { AskHostK7 } from "../AskHostK7";
import { m } from "../paraglide/messages.js";
import { findAccountFiles } from "../settings/accountFiles";
import { useDesktopEngine } from "../settings/desktopEngine";
import { engineSlots, ownedEngines } from "../settings/engineSlots";
import type { PublicEngine } from "../types";
import { type AccountFiles, accountFiles } from "./api";
import { HomeShell } from "./HomeFrame";
import { PageFrame } from "./PageFrame";
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
          (s) => `${s.module.split("/").pop()}${s.bypass ? " (bypass)" : ""}`,
        ),
      ].join(" → ");
  // The bench plays it live, so a chain that cannot run live is edited nowhere: the reason says why.
  const editTitle = chain.liveRefusal
    ? m["frozen.notLive"]({ why: chain.liveRefusal })
    : m["frozen.editOnBench"]();
  return (
    <li className="flex items-center gap-3 rounded px-2 py-2 hover:bg-muted/40">
      {chain.cold ? (
        <Snowflake className="size-4 shrink-0 text-sky-400" />
      ) : (
        <Radio className="size-4 shrink-0 text-muted-foreground" />
      )}
      <span className="min-w-0 flex-1">
        <span className="block truncate font-mono text-sm">{chain.ref}</span>
        <span
          className={`block truncate text-xs ${chain.unreadable ? "text-destructive" : "text-muted-foreground"}`}
          title={summary}
        >
          {summary}
        </span>
        <span className="block text-[11px] text-muted-foreground">
          {chain.liveRefusal
            ? m["frozen.notLive"]({ why: chain.liveRefusal })
            : m["frozen.live"]()}
          {slot && (
            <span className="ml-2 rounded bg-muted px-1.5">
              {slot === "enabled"
                ? m["frozen.slotEnabled"]()
                : m["frozen.slotKept"]()}
            </span>
          )}
        </span>
      </span>
      {onEdit && (
        <Button
          size="icon-xs"
          variant="ghost"
          disabled={!!chain.liveRefusal}
          onClick={onEdit}
          aria-label={editTitle}
          title={editTitle}
        >
          <Pencil className="size-3.5" />
        </Button>
      )}
      <Button
        size="icon-xs"
        variant="ghost"
        onClick={onCopy}
        aria-label={m["frozen.duplicate"]()}
        title={m["frozen.duplicate"]()}
      >
        <Copy className="size-3.5" />
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
          size="icon-xs"
          variant="ghost"
          onClick={onDelete}
          aria-label={m["frozen.delete"]()}
          title={m["frozen.delete"]()}
        >
          <Trash2 className="size-3.5" />
        </Button>
      )}
    </li>
  );
}

export function FrozenChainsPage() {
  // `undefined` while asked, `null` when no file service answers.
  const [files, setFiles] = useState<AccountFiles | null | undefined>(
    undefined,
  );
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
        showToast({ message: m["frozen.slotSaved"]({ ref: c.ref }) });
        return;
      }
      showToast({
        message: m["frozen.slotSending"]({ host: t.engine.hostname }),
      });
      await engineSlots.save(t.engine, c.ref);
      showToast({
        message: m["frozen.slotSavedOn"]({
          ref: c.ref,
          host: t.engine.hostname,
        }),
      });
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
    let live = true;
    void findAccountFiles(false).then((f) => {
      if (!live) return;
      setFiles(f);
      if (f) void reload(f);
    });
    return () => {
      live = false;
    };
  }, [reload]);

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
      })
    )?.trim();
    if (!name) return;
    if (!FROZEN_NAME.test(name)) return setError(m["frozen.badName"]());
    setError(null);
    try {
      await accountFiles.copyFrozenChain(files, c.ref, name);
      await reload(files);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <HomeShell>
      <AskHostK7 />
      <Toaster dismissLabel={m["common.dismiss"]()} />
      <PageFrame
        title={m["frozen.title"]()}
        summary={m["frozen.summary"]()}
        actions={
          files ? (
            <Button size="sm" onClick={() => openBench(null)}>
              <Plus className="size-3.5" /> {m["frozen.new"]()}
            </Button>
          ) : null
        }
      >
        {files === undefined || (files && chains === null && !error) ? (
          <div className="flex items-center gap-2 text-muted-foreground text-xs">
            <Loader2 className="size-3.5 animate-spin" /> {m["libs.reading"]()}
          </div>
        ) : files === null ? (
          <p className="text-muted-foreground text-sm">
            {m["frozen.noServer"]()}
          </p>
        ) : (
          <div className="space-y-6">
            {error && <p className="text-destructive text-sm">{error}</p>}
            <section>
              <h2 className="mb-1 font-medium text-sm">{m["frozen.mine"]()}</h2>
              {mine.length === 0 ? (
                <p className="text-muted-foreground text-xs">
                  {m["frozen.mineEmpty"]()}
                </p>
              ) : (
                <ul>
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
                </ul>
              )}
              <p className="mt-2 text-muted-foreground text-xs">
                {m["frozen.howToUse"]()}
              </p>
            </section>
            <section>
              <h2 className="mb-1 font-medium text-sm">{m["frozen.cold"]()}</h2>
              {cold.length === 0 ? (
                <p className="text-muted-foreground text-xs">
                  {m["frozen.coldEmpty"]()}
                </p>
              ) : (
                <ul>
                  {cold.map((c) => (
                    <Row
                      key={c.ref}
                      chain={c}
                      onCopy={() => void duplicate(c)}
                    />
                  ))}
                </ul>
              )}
            </section>
          </div>
        )}
      </PageFrame>
    </HomeShell>
  );
}
