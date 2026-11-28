// The page's half of the project address: the origin joined to the path Rust builds; the grammar is song-schema's (address_tests.rs).

import { describe, expect, it } from "vitest";
import {
  externalUrl,
  LAN_REPO,
  lanSurfaceUrl,
  launchUrl,
  localProjectUrl,
  parseExternal,
  parseLanSurface,
  parseLaunch,
  projectApp,
} from "./projects.ts";

const ORIGIN = "https://st0.priv.frediver.se";

describe("the origin joined to an address", () => {
  it("names k7 for a project folder and nothing for another", () => {
    expect(projectApp("mysong.k7")).toBe("k7");
    expect(projectApp("mysong")).toBe(null);
  });

  it("sends a project to k7's origin, whatever path the URL it is handed carries", () => {
    expect(launchUrl(ORIGIN, "band", "le clip.k7", "k7")).toBe(
      `${ORIGIN}/band/le%20clip.k7`,
    );
    expect(launchUrl(`${ORIGIN}/projects?x=1`, "b", "c.k7", "k7")).toBe(
      `${ORIGIN}/b/c.k7`,
    );
  });

  it("builds a surface's and a local project's address, which parse back", () => {
    const url = externalUrl(
      ORIGIN,
      { repo: "band", project: "winter.k7" },
      "le desk",
      "k7",
    );
    expect(url).toContain("/@external/le%20desk");
    expect(parseExternal(new URL(url).pathname, "k7", "/")).toBe("le desk");
    const local = localProjectUrl(ORIGIN, "mes projets", "hiver.k7", "k7");
    expect(local).toBe(`${ORIGIN}/@local/mes%20projets/hiver.k7`);
    expect(parseLaunch(new URL(local).pathname, "k7", "/")?.repo).toBe(
      "@local",
    );
  });

  it("builds a surface of another device's address, which parses back", () => {
    const where = { engineId: "e-9f", workspace: "band", project: "mix.k7" };
    const url = new URL(lanSurfaceUrl("https://x.test", where, "desk", "k7"));
    expect(url.pathname).toBe(`/${LAN_REPO}/e-9f/band/mix.k7/@external/desk`);
    expect(parseLanSurface(url.pathname, "k7", "/")).toEqual({
      ...where,
      surface: "desk",
    });
  });
});
