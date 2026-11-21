// placeResource.ts — the lane menu's "Place <resource>" rows: the keyboard and touch road to what a drag does on a desktop.
// They are appended to the menu once the resources arrive, so a lane's menu never waits on the listing.

import { sortForPeople } from "@studio/web-protocol/tree";
import { api, mediaUrl } from "./api";
import type { MenuCtx, MenuItem } from "./contextMenu";
import { m } from "./paraglide/messages.js";

/** The most rows added to one menu: a project with hundreds of imports is placed from the panel. */
const MAX_ROWS = 24;

/** A resource of the lane's own kind, as a menu row that runs `onDropResource` rather than a command line. */
export async function resourceRows(
  ctx: MenuCtx,
  /** The caret's timecode, where an audio clip lands; null puts it at the start. */
  tc: string | null,
): Promise<MenuItem[]> {
  const video = ctx.kind === "picture-lane";
  if (ctx.kind !== "track" && !video) return [];
  const lane = video ? ctx.lane : ctx.id;
  const kind = video ? "video" : "audio";
  try {
    const { resources } = await api.resources();
    return sortForPeople(
      resources.filter((r) => r.kind === kind && !r.missing),
      (r) => r.id,
    )
      .slice(0, MAX_ROWS)
      .map((r) => ({
        label: r.id,
        description: m["menu.placeResourceHelp"]({ id: r.id }),
        command: `${video ? "picture add" : "event add"} ${lane} ${r.id}`,
        complete: true,
        verb: video ? "picture add" : "event add",
        sentence: m["menu.placeResource"]({ id: r.id }),
        place: {
          lane,
          kind,
          id: r.id,
          url: mediaUrl(r.preview ?? r.source),
          tc: tc ?? "1:1",
          seconds: 0,
        },
      }));
  } catch {
    return [];
  }
}
