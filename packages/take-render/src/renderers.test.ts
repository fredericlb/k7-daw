// renderers.test.ts — a render this service cannot make itself is made on an engine's MACHINE, over a
// real tunnel: the clip it plays is on the machine before the bounce, and the mix and stems the machine
// wrote land in the caller's folder under their own names. The "engine" is a stand-in job; what is
// pinned is the route, not the sound.

import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { TunnelAgent } from "@studio/engine-tunnel/agent";
import {
  describeCore,
  type HubTarget,
  startHub,
} from "@studio/engine-tunnel/test-core";
import type { ProjectDoc } from "@studio/song/project";
import { AUTO_RENDERS_HOME_DIR } from "@studio/song-schema/moduleTypes";
import { afterEach, beforeEach, expect, it } from "vitest";
import { machineRenderer } from "./renderers.ts";

const until = async (ok: () => Promise<boolean>, ms = 5000): Promise<void> => {
  const end = Date.now() + ms;
  while (!(await ok())) {
    if (Date.now() > end) throw new Error("timed out");
    await new Promise((r) => setTimeout(r, 10));
  }
};

describeCore("a render made on an engine's machine", () => {
  let hub: HubTarget;
  let agent: TunnelAgent;
  let home: string;
  let workspace: string;
  let jobsUrl: string;
  const savedHome = process.env.STUDIO_HOME;

  beforeEach(async () => {
    home = mkdtempSync(join(tmpdir(), "rr-home-"));
    workspace = join(home, "workspace");
    mkdirSync(workspace, { recursive: true });
    process.env.STUDIO_HOME = home;
    hub = await startHub();
    agent = new TunnelAgent({
      registryUrl: hub.agentUrl,
      id: "server",
      token: "t",
      enginePort: 1,
      cacheDir: mkdtempSync(join(tmpdir(), "rr-cache-")),
      log: () => {},
      jobs: {
        // Stands in for the engine: the clip must be HERE, and every file is written under `outDir`.
        render(req, emit) {
          const doc = req.doc as { tracks: { clips: { source: string }[] }[] };
          const clip = readFileSync(doc.tracks[0].clips[0].source, "utf8");
          const outDir = String(req.outDir);
          mkdirSync(outDir, { recursive: true });
          writeFileSync(join(outDir, "mix.wav"), `mix of ${clip}`);
          const stems = req.stems
            ? [{ name: "Gtr", file: join(outDir, "gtr.wav") }]
            : [];
          for (const s of stems) writeFileSync(s.file, `stem of ${clip}`);
          emit({ type: "rendered", mix: join(outDir, "mix.wav"), stems });
        },
      },
    });
    agent.start();
    await until(() => hub.has("server"));
    const roots = [
      { remote: workspace, name: "ws", writable: true },
      {
        remote: join(home, AUTO_RENDERS_HOME_DIR),
        name: "renders",
        writable: true,
      },
    ];
    jobsUrl = `ws://127.0.0.1:${hub.port}${await hub.bindLocal("server", roots, "jobs")}`;
  });

  afterEach(async () => {
    agent.stop();
    await hub.stop();
    if (savedHome === undefined) delete process.env.STUDIO_HOME;
    else process.env.STUDIO_HOME = savedHome;
  });

  const doc = (): ProjectDoc => {
    const source = join(workspace, "riff.wav");
    writeFileSync(source, "riff");
    return { tracks: [{ clips: [{ source }] }] } as unknown as ProjectDoc;
  };

  it("brings the mix and the stems back into the caller's folder", async () => {
    const out = mkdtempSync(join(tmpdir(), "rr-out-"));
    const manifest = await machineRenderer(() => jobsUrl)(doc(), out, {
      stems: true,
    });
    expect(manifest).toEqual({
      mix: "mix.wav",
      stems: [{ name: "Gtr", file: "gtr.wav" }],
    });
    expect(readFileSync(join(out, "mix.wav"), "utf8")).toBe("mix of riff");
    expect(readFileSync(join(out, "gtr.wav"), "utf8")).toBe("stem of riff");
  });

  it("leaves nothing in the render cache once the files are moved", async () => {
    await machineRenderer(() => jobsUrl)(
      doc(),
      mkdtempSync(join(tmpdir(), "rr-out-")),
    );
    const jobs = join(home, AUTO_RENDERS_HOME_DIR, "jobs");
    expect(existsSync(jobs) ? readdirSync(jobs) : []).toEqual([]);
  });

  it("refuses a base folder, which names the machine's disk and not this one", async () => {
    await expect(
      machineRenderer(() => jobsUrl)(doc(), tmpdir(), { base: "/somewhere" }),
    ).rejects.toThrow(/base folder/);
  });

  it("says so when there is no machine to ask", async () => {
    await expect(machineRenderer(() => null)(doc(), tmpdir())).rejects.toThrow(
      /no engine machine/,
    );
  });
});
