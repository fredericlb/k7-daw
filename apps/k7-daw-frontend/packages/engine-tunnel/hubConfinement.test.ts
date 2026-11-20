// hubConfinement.test.ts — the hub's files attacked by a bare agent sending what no real agent would: dot
// segments, encoded and foreign separators, a prefix of the root, links out of the tree, Windows device
// names and stream suffixes, uploads that stop, collide or overrun. The cases of PathMap_test.cpp and
// segments.test.ts, aimed at the server side; run against either hub (testCore.ts). docs/tunnel.md.

import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, sep } from "node:path";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket } from "ws";
import type { TunnelRoot } from "./frames.ts";

import {
  type BareAgent,
  bareAgent,
  describeCore,
  type HubTarget,
  startHub,
  type UploadLimits,
  until,
} from "./testCore.ts";

const WIN = process.platform === "win32";

/** The tree the attacks aim at: a root, a secret beside it, links out, a library whose clone is a link. */
function tree() {
  const base = mkdtempSync(join(tmpdir(), "confine-"));
  const R = join(base, "ws");
  mkdirSync(join(R, "song"), { recursive: true });
  writeFileSync(join(R, "song", "a.wav"), "RIFF-a");
  mkdirSync(join(base, "outside"));
  writeFileSync(join(base, "outside", "secret"), "SECRET");
  mkdirSync(join(base, "wsx"));
  writeFileSync(join(base, "wsx", "secret"), "SECRET");
  // A junction, which Windows lets anyone make; a symlink elsewhere.
  symlinkSync(join(base, "outside"), join(R, "escape"), "junction");
  mkdirSync(join(base, "checkout", "wavetables"), { recursive: true });
  writeFileSync(join(base, "checkout", "wavetables", "basic.wav"), "RIFF-b");
  symlinkSync(
    join(base, "outside"),
    join(base, "checkout", "escape"),
    "junction",
  );
  mkdirSync(join(base, "edits"));
  symlinkSync(join(base, "checkout"), join(base, "edits", "k7"), "junction");
  return { base, R, L: join(base, "edits") };
}

/** A hub with a bare agent dialled in, and a channel of `roots` open on it. */
async function channelOn(hub: HubTarget, roots: TunnelRoot[]) {
  const agent = await bareAgent(hub.agentUrl);
  await until(() => hub.has("e1"));
  const path = await hub.bindLocal("e1", roots);
  const client = new WebSocket(`ws://127.0.0.1:${hub.port}${path}`);
  await new Promise((r) => client.once("open", r));
  const { ch } = await agent.opened();
  return { agent, client, ch };
}

describeCore("the hub's confinement, attacked", () => {
  let t: ReturnType<typeof tree>;
  let hub: HubTarget;
  let agent: BareAgent;
  let client: WebSocket;
  let ch: number;

  beforeEach(async () => {
    t = tree();
    hub = await startHub();
    ({ agent, client, ch } = await channelOn(hub, [
      { remote: t.R, name: "ws", writable: true },
      { remote: t.L, name: "edits", writable: false, perChild: true },
    ]));
  });
  afterEach(async () => {
    client.terminate();
    agent.close();
    await hub.stop();
    rmSync(t.base, { recursive: true, force: true });
  });

  /** Paths no channel may reach, whatever the server's system. */
  const attacks = () => [
    `${t.R}${sep}..${sep}outside${sep}secret`,
    `${t.R}${sep}.${sep}song${sep}a.wav`,
    `${t.R}${sep}song${sep}..${sep}..${sep}outside${sep}secret`,
    `${t.base}${sep}wsx${sep}secret`,
    `song${sep}a.wav`,
    "",
    `${t.R}${sep}escape${sep}secret`,
    `${t.R}${sep}..%2Foutside%2Fsecret`,
    `${t.R}${sep}%2e%2e${sep}outside${sep}secret`,
    `${t.R}/..\\..\\outside\\secret`,
    `${t.R}${sep}CON`,
    `${t.R}${sep}nul.wav`,
    `${t.R}${sep}COM1.txt`,
    `${t.R}${sep}AUX .wav`,
    `${t.R}${sep}CONIN$`,
    `${t.R}${sep}lpt¹`,
    `${t.R}${sep}song${sep}a\tb.wav`,
    `${t.R}${sep}song${sep}a.wav:stream`,
    `${t.L}${sep}k7${sep}escape${sep}secret`,
    ...(WIN
      ? [
          `${t.R}${sep}song${sep}a.wav.`,
          `${t.R}${sep}song${sep}a.wav `,
          `${t.R}${sep}song${sep}a.wav::$DATA`,
          `${t.R}${sep}..\\outside\\secret`,
        ]
      : [`${t.R}//song/a.wav`]),
  ];

  it("stats nothing outside the roots, and what is inside", async () => {
    const legit = [
      t.R,
      `${t.R}${sep}song${sep}a.wav`,
      `${t.L}${sep}k7${sep}wavetables${sep}basic.wav`,
    ];
    const got = await agent.stat(ch, [...legit, ...attacks()]);
    expect(got.slice(0, 3).map((e) => e.kind)).toEqual(["dir", "file", "file"]);
    for (const e of got.slice(3))
      expect([e.path, e.kind]).toEqual([e.path, "missing"]);
  });

  it("serves no byte outside the roots, and a file inside whole", async () => {
    expect(await agent.get(ch, `${t.R}${sep}song${sep}a.wav`)).toEqual(
      Buffer.from("RIFF-a"),
    );
    for (const p of attacks()) {
      const r = await agent.get(ch, p);
      expect([p, Buffer.isBuffer(r)]).toEqual([p, false]);
    }
  });

  it("takes no upload outside a writable root, nor one beside a link out", async () => {
    const ok = await agent.put(ch, `${t.R}${sep}takes${sep}t1.wav`, [
      Buffer.from("TAKE"),
    ]);
    expect(ok.error).toBeUndefined();
    expect(readFileSync(join(t.R, "takes", "t1.wav"), "utf8")).toBe("TAKE");
    // An encoded separator is a name like any other: it lands inside the root, under that name.
    const encoded = await agent.put(ch, `${t.R}${sep}..%2Foutside%2Fx`, [
      Buffer.from("E"),
    ]);
    expect(encoded.error).toBeUndefined();
    expect(readFileSync(join(t.R, "..%2Foutside%2Fx"), "utf8")).toBe("E");
    const refused = [
      ...attacks().filter((p) => !p.includes("%")),
      `${t.R}${sep}escape${sep}planted`,
      `${t.L}${sep}k7${sep}wavetables${sep}new.wav`,
      `${t.base}${sep}elsewhere.wav`,
    ];
    for (const p of refused) {
      const r = await agent.put(ch, p, [Buffer.from("X")]);
      expect([p, typeof r.error]).toEqual([p, "string"]);
    }
    expect(readdirSync(join(t.base, "outside"))).toEqual(["secret"]);
    expect(readFileSync(join(t.base, "outside", "secret"), "utf8")).toBe(
      "SECRET",
    );
    expect(readdirSync(join(t.R, "takes"))).toEqual(["t1.wav"]);
  });

  it("leaves no part file and no target when the tunnel drops mid-upload", async () => {
    const target = join(t.R, "song", "half.wav");
    agent.putChunk(ch, 7, target, Buffer.from("half"), false);
    await until(() =>
      readdirSync(join(t.R, "song")).some((f) => f.endsWith(".part")),
    );
    agent.close();
    await until(
      () => !readdirSync(join(t.R, "song")).some((f) => f.endsWith(".part")),
    );
    expect(existsSync(target)).toBe(false);
  });

  it("keeps two uploads of one path apart, each in a part of its own", async () => {
    const target = join(t.R, "song", "twice.wav");
    agent.putChunk(ch, 1, target, Buffer.from("AAAA"), false);
    agent.putChunk(ch, 2, target, Buffer.from("BBBB"), false);
    agent.putChunk(ch, 1, target, Buffer.from("AAAA"), true);
    agent.putChunk(ch, 2, target, Buffer.from("BBBB"), true);
    await until(() => agent.replies("put-reply").length === 2);
    // One may lose the rename on Windows while the other holds the target: never both, never a mix.
    expect(
      agent.replies("put-reply").filter((r) => !r.error).length,
    ).toBeGreaterThan(0);
    expect(["AAAAAAAA", "BBBBBBBB"]).toContain(readFileSync(target, "utf8"));
    expect(
      readdirSync(join(t.R, "song")).filter((f) => f.endsWith(".part")),
    ).toEqual([]);
  });
});

describeCore("what one tunnel may upload", () => {
  let dir: string;
  let hub: HubTarget;
  let agent: BareAgent;
  let client: WebSocket;
  let ch: number;

  const open = async (uploads: Partial<UploadLimits>) => {
    hub = await startHub({ uploads });
    ({ agent, client, ch } = await channelOn(hub, [
      { remote: dir, name: "ws", writable: true },
    ]));
  };
  const put = (req: number, name: string, bytes: number, last: boolean) =>
    agent.putChunk(ch, req, join(dir, name), Buffer.alloc(bytes, 7), last);
  const replies = () => agent.replies("put-reply");

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "hub-uploads-"));
  });
  afterEach(async () => {
    client.terminate();
    agent.close();
    await hub.stop();
    rmSync(dir, { recursive: true, force: true });
  });

  it("lands a file whole, under its own name", async () => {
    await open({});
    put(1, "take.wav", 5, false);
    put(1, "take.wav", 5, true);
    await until(() => replies().length > 0, 5000);
    expect(replies()).toMatchObject([{ req: 1 }]);
    expect(replies()[0].error).toBeUndefined();
    expect(readFileSync(join(dir, "take.wav")).length).toBe(10);
    expect(readdirSync(dir).filter((f) => f.endsWith(".part"))).toEqual([]);
  });

  it("refuses an upload past the number open at once", async () => {
    await open({ maxOpen: 2 });
    put(1, "a", 1, false);
    put(2, "b", 1, false);
    put(3, "c", 1, false);
    await until(() => replies().length > 0, 5000);
    expect(replies()).toMatchObject([{ req: 3, error: expect.any(String) }]);
  });

  it("aborts an upload past its size, deletes its part file and ignores its later chunks", async () => {
    await open({ maxBytes: 10 });
    put(1, "big", 8, false);
    // The refusal is told before the chain has made its part file: overrun only once it exists.
    await until(() => readdirSync(dir).some((f) => f.endsWith(".part")), 5000);
    put(1, "big", 8, false);
    put(1, "big", 8, true);
    await until(
      () => replies().length > 0 && readdirSync(dir).length === 0,
      5000,
    );
    expect(replies()).toMatchObject([{ req: 1, error: expect.any(String) }]);
    expect(readdirSync(dir)).toEqual([]);
  });

  it("aborts an upload that went quiet", async () => {
    await open({ idleMs: 300 });
    put(1, "slow", 4, false);
    await until(() => readdirSync(dir).some((f) => f.endsWith(".part")), 5000);
    await until(
      () => replies().length > 0 && readdirSync(dir).length === 0,
      5000,
    );
    expect(replies()).toMatchObject([{ req: 1, error: expect.any(String) }]);
    expect(readdirSync(dir)).toEqual([]);
  });

  it("stops at the hourly budget of the tunnel", async () => {
    await open({ bytesPerHour: 12 });
    put(1, "a", 8, true);
    put(2, "b", 8, true);
    await until(() => replies().length >= 2, 5000);
    expect(replies().find((r) => r.req === 2)?.error).toBeDefined();
    expect(replies().find((r) => r.req === 1)?.error).toBeUndefined();
  });
});
