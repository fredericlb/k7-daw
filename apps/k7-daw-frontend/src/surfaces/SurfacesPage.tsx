// SurfacesPage.tsx — `/surfaces`, a page of the home: for every project open on the network, the surfaces THIS
// device can open. A row leads to the surface alone; it is a hard navigation, since a surface mounts under a
// project basename the home's router cannot reach. See ../../docs/external.md.

import { Button } from "@studio/ui/button";
import { EmptyState } from "@studio/ui/empty-state";
import { RefreshCw, TabletSmartphone } from "@studio/ui/icons";
import { Row, RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
import { useCallback, useEffect, useMemo, useState } from "react";
import { parts } from "../i18nParts";
import { m } from "../paraglide/messages.js";
import { HomeShell } from "../projects/HomeFrame";
import { PageFrame } from "../projects/PageFrame";
import { deviceSurfaceDrawer } from "../surfacePanes";
import { pollWhileVisible } from "../visiblePoll";
import { drawersReady } from "./drawers";
import { type LanPeer, lanApi } from "./lanApi";
import { SurfaceIcon, typeLabel } from "./surfaceMeta";
import {
  type Blocked,
  type DeviceGroup,
  type PeerState,
  type SurfaceRow,
  surfaceGroups,
} from "./surfaceRows";

/** A peer's list is asked again this often; its hello answers at once or not at all. */
const POLL_MS = 5000;

/** The sentence for a reason a row cannot open. */
export function blockedText(b: Blocked, device: string): string {
  switch (b.why) {
    case "notAnswering":
      return m["surfaces.notAnswering"]();
    case "notTrusted":
      return m["surfaces.notTrusted"]({ device });
    case "update":
      return m["surfaces.update"]({ device: b.device });
    case "windowOnly":
      return m["surfaces.drawnInWindow"]();
  }
}

export function SurfacesPage() {
  // `null` until the first answer: "nothing found" is drawn only once that is the answer.
  const [peers, setPeers] = useState<PeerState[] | null>(null);
  const [selfName, setSelfName] = useState("");
  const [selfId, setSelfId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Which types this device can draw is the registry's answer, which the editor's chunk fills (drawers.ts).
  const [drawers, setDrawers] = useState(false);
  useEffect(() => {
    let live = true;
    void drawersReady().then(() => live && setDrawers(true));
    return () => {
      live = false;
    };
  }, []);

  /** The list, then each device's hello together, so a device appears whole and the list does not grow line by line. */
  const load = useCallback(async () => {
    setBusy(true);
    try {
      const [{ peers: found }, self] = await Promise.all([
        lanApi.peers(),
        lanApi.self().catch(() => null),
      ]);
      setSelfName(self?.name ?? "");
      setSelfId(self?.engineId ?? null);
      const states = await Promise.all(
        found.map(async (peer: LanPeer): Promise<PeerState> => {
          if (!peer.reached || !peer.trusted) return { peer, hello: null };
          return {
            peer,
            hello: await lanApi.hello(peer.engineId).catch(() => null),
          };
        }),
      );
      setPeers(states);
    } catch {
      // This front answers no LAN family (a server): the same page as a network with nobody on it.
      setPeers([]);
    } finally {
      setBusy(false);
    }
  }, []);
  useEffect(() => {
    void load();
    return pollWhileVisible(load, POLL_MS);
  }, [load]);

  const groups = useMemo(
    () =>
      surfaceGroups({
        peers: peers ?? [],
        selfId,
        selfName: selfName || m["surfaces.thisDevice"](),
        drawable: (type) => !!deviceSurfaceDrawer(type),
        origin: window.location.origin,
      }),
    [peers, selfId, selfName],
  );

  return (
    <HomeShell>
      <PageFrame
        title={m["home.surfaces"]()}
        summary={
          peers ? m["surfaces.summary"]({ n: groups.length }) : undefined
        }
        actions={
          <Button
            size="icon"
            variant="glass"
            onClick={() => void load()}
            title={m["surfaces.refresh"]()}
            aria-label={m["surfaces.refresh"]()}
          >
            <RefreshCw className={busy ? "size-5 animate-spin" : "size-5"} />
          </Button>
        }
      >
        {peers === null || !drawers ? (
          <div className="type-secondary flex items-center gap-2 text-fg-2">
            <Spinner />
            {m["surfaces.looking"]()}
          </div>
        ) : groups.length === 0 ? (
          <EmptyState
            glyph={<TabletSmartphone />}
            title={m["surfaces.noDevice"]()}
          >
            {m["surfaces.noDeviceBody"]()}
          </EmptyState>
        ) : (
          groups.map((g) => <Device key={g.engineId} group={g} />)
        )}
      </PageFrame>
    </HomeShell>
  );
}

/** One device: its name over its project, and a row per surface or the one line that says why there is none. */
function Device({ group: g }: { group: DeviceGroup }) {
  return (
    <section aria-label={g.device} className="flex flex-col gap-2">
      <h2 className="flex min-w-0 items-baseline gap-2 px-1">
        <span className="type-caps truncate text-fg-3">{g.device}</span>
        {g.project && (
          <span className="type-status truncate text-fg-3">{g.project}</span>
        )}
      </h2>
      <RowGroup>
        {g.blocked ? (
          <Row className="text-fg-2">{blockedText(g.blocked, g.device)}</Row>
        ) : g.rows.length === 0 ? (
          <Row
            className="text-fg-2"
            detail={
              g.project
                ? parts((h) => m["surfaces.noSurfacesBody"](h), {
                    folder: <code>surfaces/</code>,
                  })
                : undefined
            }
          >
            {g.project
              ? m["surfaces.noSurfaces"]({ device: g.device })
              : m["surfaces.noProject"]({ device: g.device })}
          </Row>
        ) : (
          g.rows.map((r) => <Entry key={r.key} row={r} device={g.device} />)
        )}
      </RowGroup>
    </section>
  );
}

/** One surface: a link when it opens, the same row with its reason when it does not. */
function Entry({ row: r, device }: { row: SurfaceRow; device: string }) {
  const also = r.alsoOn.length
    ? m["surfaces.alsoOn"]({ device: r.alsoOn.join(", ") })
    : null;
  const reason = r.blocked ? blockedText(r.blocked, device) : also;
  const row = (
    <Row
      icon={<SurfaceIcon type={r.type} />}
      detail={typeLabel(r.type)}
      disclosure={!!r.href}
      className={r.href ? undefined : "text-fg-3"}
      trailing={
        reason ? (
          <span className="type-status shrink-0 text-fg-3">{reason}</span>
        ) : undefined
      }
    >
      {r.id}
    </Row>
  );
  // Same tab, an anchor: an installed app has no tab to come back from, and a surface cannot use the router.
  return r.href ? (
    <a
      href={r.href}
      className="block outline-none transition-colors hover:bg-on-fill focus-visible:shadow-[inset_0_0_0_2px_var(--k7-focus-line)]"
    >
      {row}
    </a>
  ) : (
    row
  );
}
