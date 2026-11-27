// LanSurfaceGate.tsx — the gate of a surface that belongs to ANOTHER device: the account, that device's hello, and the
// file transport pointed at its door through this device's own front. It opens no project, takes no claim and
// sends nothing to the engine: the main device already holds the project, and opening it from here would
// run the open sequence there and could interrupt what is playing. See ../../docs/external.md.

import { deviceName } from "@studio/engine-browser/pairing";
import { configureFileApi } from "@studio/project-shell/fileApi";
import { GateWait } from "@studio/project-shell/progress";
import { Button } from "@studio/ui/button";
import { House, Unplug } from "@studio/ui/icons";
import type { LanSurface } from "@studio/web-protocol/projects";
import { surfacesUrl } from "@studio/web-protocol/projects";
import { useEffect, useMemo, useState } from "react";
import { AskHostK7 } from "../AskHostK7";
import { api } from "../api";
import { m } from "../paraglide/messages.js";
import { drawersReady } from "../surfaces/drawers";
import { lanApi } from "../surfaces/lanApi";
import { pollWhileVisible } from "../visiblePoll";
import { ExternalSurface } from "./ExternalSurface";

/** The project is asked after again this often: a project closed on the other device ends this screen. */
const WATCH_MS = 5000;

type State =
  | { k: "loading" }
  | { k: "ready"; device: string; self: string; userEmail: string }
  | { k: "closed"; device: string }
  | { k: "gone" };

/** The engine socket of that device, through this device's own front: the page speaks to loopback only. */
export function lanEngineUrl(engineId: string): string {
  const scheme = window.location.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${window.location.host}/api/lan/${encodeURIComponent(engineId)}/engine`;
}

export function LanSurfaceGate({ lan }: { lan: LanSurface }) {
  const [state, setState] = useState<State>({ k: "loading" });
  // One URL, asked on every dial: a relayed socket needs no ticket, the front holds the session.
  const engineUrl = useMemo(() => lanEngineUrl(lan.engineId), [lan.engineId]);

  useEffect(() => {
    let live = true;
    let ready = false;
    const same = (p: { workspace: string; path: string } | null) =>
      !!p && p.workspace === lan.workspace && p.path === lan.project;
    const look = async () => {
      try {
        const hello = await lanApi.hello(lan.engineId);
        if (!live) return;
        const device = hello.device.name;
        if (!same(hello.project)) return setState({ k: "closed", device });
        if (ready) return;
        const [me, self] = await Promise.all([
          api.getMe(),
          lanApi.self().catch(() => null),
          // The drawers of every surface type, which a project's own address gets from the editor's chunk.
          drawersReady(),
        ]);
        if (!live) return;
        const userEmail = me?.email || me?.sub || "unknown";
        // The files of the OTHER device's project, through its door; the relay holds the token, so this one is a placeholder.
        configureFileApi({
          base: `/api/lan/${encodeURIComponent(lan.engineId)}/files`,
          userEmail,
          engineToken: "lan",
          engineId: lan.engineId,
          workspace: lan.workspace,
        });
        ready = true;
        setState({
          k: "ready",
          device,
          // A device with no engine of its own has no name to give: the kind of device and browser it is stands in.
          self: self?.name || deviceName(),
          userEmail,
        });
      } catch {
        // A device that does not answer yet is waited for; one that never did is said so after the first miss.
        if (live && !ready) setState({ k: "gone" });
      }
    };
    void look();
    const stop = pollWhileVisible(look, WATCH_MS);
    return () => {
      live = false;
      stop();
    };
  }, [lan]);

  if (state.k === "loading") return <GateWait label={m["startup.opening"]()} />;
  if (state.k === "closed" || state.k === "gone")
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 bg-ground p-6 text-center text-fg-1">
        <Unplug className="size-8 text-fg-3" aria-hidden="true" />
        <p className="max-w-sm text-sm">
          {state.k === "closed"
            ? m["surfaces.projectClosed"]({ device: state.device })
            : m["surfaces.noAddress"]()}
        </p>
        {state.k === "closed" && (
          <p className="max-w-sm text-xs opacity-70">
            {m["surfaces.projectClosedBody"]()}
          </p>
        )}
        <Button asChild variant="glass">
          <a href={surfacesUrl(window.location.origin, "k7")}>
            <House className="size-4" />
            {m["extsrf.home"]()}
          </a>
        </Button>
      </div>
    );
  return (
    <>
      <ExternalSurface
        key={`${lan.engineId}/${lan.workspace}/${lan.project}/${lan.surface}`}
        name={lan.surface}
        project={lan.project}
        engineUrl={engineUrl}
        userEmail={state.userEmail}
        lan={{ engineId: lan.engineId, device: state.self }}
      />
      <AskHostK7 />
    </>
  );
}
