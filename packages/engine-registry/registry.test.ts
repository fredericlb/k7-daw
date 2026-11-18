// registry.test.ts — the claim's three rules, and the one property that makes takeover
// survivable: a renewal is not an open. docs/claim.md is the prose; this is the contract.
//
// Every bug this pins is a LIFECYCLE bug, the blind spot docs/audit-moteur.md §E names for this
// area's tests: a pure policy is easy to cover, what happens to a claim over time is not.

import { ENGINE_ASSIGN_TTL_MS } from "@studio/web-protocol";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  type AsyncRegistry,
  type CoreTarget,
  describeCore,
  startCore,
} from "./testCore.ts";

const beat = async (reg: AsyncRegistry, id: string) =>
  await reg.upsert({
    id,
    token: `tok-${id}`,
    hostname: id,
    machineFingerprint: `fp-${id}`,
    addresses: {
      loopback: "127.0.0.1",
      lanIps: [],
      port: 8765,
    },
  });

/** A successful claim's secret, or a failure — narrowed, since the union is the point. */
const grant = (r: Awaited<ReturnType<AsyncRegistry["claim"]>>) => {
  if (!r.ok) throw new Error(`expected a grant, got heldBy=${r.heldBy}`);
  return r;
};

describeCore("the engine claim", () => {
  let core: CoreTarget;
  let reg: AsyncRegistry;

  beforeEach(async () => {
    core = await startCore();
    core.clock.fake();
    reg = core.reg;
    await beat(reg, "e1");
    await beat(reg, "e2");
  });
  afterEach(async () => {
    core.clock.real();
    await core.stop();
  });

  it("grants an open, and renews it with the secret it gave back", async () => {
    const g = grant(
      await reg.claim("e1", { project: "band/winter.k7", holder: "a" }),
    );
    expect(g.secret).toBeTruthy();
    expect(g.tookOver).toBeUndefined();

    const again = grant(
      await reg.claim("e1", {
        project: "band/winter.k7",
        holder: "a",
        secret: g.secret,
      }),
    );
    expect(again.secret).toBe(g.secret); // the same hold, not a second one
    expect(await reg.engineForProject("band/winter.k7")).toBe("e1");
  });

  it("lets the newcomer take a project, and REFUSES the loser's next renewal", async () => {
    const first = grant(
      await reg.claim("e1", { project: "p", holder: "alice" }),
    );
    const second = await reg.claim("e1", { project: "p", holder: "bob" });
    expect(second.ok).toBe(true);
    expect(grant(second).tookOver).toEqual({ holder: "alice", project: "p" });

    // The whole reason a claim carries a secret: alice renewing must NOT win it back, or the two trade the project for ever.
    const refused = await reg.claim("e1", {
      project: "p",
      holder: "alice",
      secret: first.secret,
    });
    expect(refused.ok).toBe(false);
    expect(refused.ok === false && refused.heldBy).toBe("bob");

    // …and bob is untouched by the refusal.
    expect(
      grant(
        await reg.claim("e1", {
          project: "p",
          holder: "bob",
          secret: grant(second).secret,
        }),
      ).secret,
    ).toBe(grant(second).secret);
  });

  it("moves a project between engines rather than letting it run on two", async () => {
    const onE1 = grant(
      await reg.claim("e1", { project: "p", holder: "alice" }),
    );
    grant(await reg.claim("e2", { project: "p", holder: "bob" }));

    expect(await reg.engineForProject("p")).toBe("e2");
    expect(
      (
        await reg.claim("e1", {
          project: "p",
          holder: "alice",
          secret: onE1.secret,
        })
      ).ok,
    ).toBe(false);
  });

  it("refuses a SECOND project on one engine — the file service holds only one", async () => {
    const song = grant(
      await reg.claim("e1", { project: "band/winter.k7", holder: "a" }),
    );
    grant(await reg.claim("e1", { project: "band/leclip.k7", holder: "b" }));

    // Not tidiness: the file service keeps a single active project per machine, so a second one clobbers it.
    const refused = await reg.claim("e1", {
      project: "band/winter.k7",
      holder: "a",
      secret: song.secret,
    });
    expect(refused.ok).toBe(false);
  });

  it("lets ONE account hold one project twice on one engine — a surface beside its editor", async () => {
    const editor = grant(
      await reg.claim("e1", { project: "band/winter.k7", holder: "a" }),
    );
    // The tablet: same account and same engine, so the same session and the same loaded project, which rule 1 has nothing to protect.
    const surface = grant(
      await reg.claim("e1", { project: "band/winter.k7", holder: "a" }),
    );
    expect(surface.tookOver).toBeUndefined();

    // Both keep renewing, and neither is told its project was opened elsewhere.
    for (const secret of [editor.secret, surface.secret])
      expect(
        grant(
          await reg.claim("e1", {
            project: "band/winter.k7",
            holder: "a",
            secret,
          }),
        ).secret,
      ).toBe(secret);
  });

  it("still takes that project from ANOTHER account on the same engine", async () => {
    const mine = grant(
      await reg.claim("e1", { project: "p", holder: "alice" }),
    );
    const theirs = await reg.claim("e1", { project: "p", holder: "bob" });
    expect(grant(theirs).tookOver).toEqual({ holder: "alice", project: "p" });
    expect(
      (
        await reg.claim("e1", {
          project: "p",
          holder: "alice",
          secret: mine.secret,
        })
      ).ok,
    ).toBe(false);
  });

  it("still moves that project when the SAME account opens it on another engine", async () => {
    // The exemption is same account AND same engine: a second machine is a second session, so the project really does move.
    const onE1 = grant(
      await reg.claim("e1", { project: "p", holder: "alice" }),
    );
    grant(await reg.claim("e2", { project: "p", holder: "alice" }));
    const refused = await reg.claim("e1", {
      project: "p",
      holder: "alice",
      secret: onE1.secret,
    });
    expect(refused.ok).toBe(false);
    if (!refused.ok) expect(refused.lost).toBe("project");
  });

  it("still evicts a DIFFERENT project of the same account on one engine", async () => {
    // Rule 2 is untouched by the exemption: one project per engine is what the file service requires, whoever is asking.
    const winter = grant(
      await reg.claim("e1", { project: "band/winter.k7", holder: "a" }),
    );
    grant(await reg.claim("e1", { project: "band/leclip.k7", holder: "a" }));
    const refused = await reg.claim("e1", {
      project: "band/winter.k7",
      holder: "a",
      secret: winter.secret,
    });
    expect(refused.ok).toBe(false);
    if (!refused.ok) {
      expect(refused.lost).toBe("engine");
      expect(refused.tookFor).toBe("band/leclip.k7");
    }
  });

  it("lets a project-less holder sit beside a project — pl8 must not evict k7", async () => {
    const song = grant(
      await reg.claim("e1", { project: "band/winter.k7", holder: "a" }),
    );
    // pl8 drives an engine and opens no project, so looking at the catalogue must not take the song away from the k7 tab beside it.
    const catalogue = grant(
      await reg.claim("e1", { project: null, holder: "a" }),
    );

    expect(
      grant(
        await reg.claim("e1", {
          project: "band/winter.k7",
          holder: "a",
          secret: song.secret,
        }),
      ).secret,
    ).toBe(song.secret);
    expect(
      grant(
        await reg.claim("e1", {
          project: null,
          holder: "a",
          secret: catalogue.secret,
        }),
      ).secret,
    ).toBe(catalogue.secret);
    expect(await reg.engineForProject("band/winter.k7")).toBe("e1");
  });

  it("forgets a hold nobody renewed, without anyone saying goodbye", async () => {
    grant(await reg.claim("e1", { project: "p", holder: "alice" }));
    expect((await reg.list())[0].assigned).toBe(true);

    // The tab crashed. Nothing releases, and that is the case the TTL exists for.
    await core.clock.advance(ENGINE_ASSIGN_TTL_MS + 1);
    await beat(reg, "e1"); // the ENGINE is still alive and beating; only the holder went
    expect((await reg.list())[0].assigned).toBe(false);
    expect(await reg.engineForProject("p")).toBeUndefined();
  });

  it("releases only the hold whose secret is presented", async () => {
    const alice = grant(
      await reg.claim("e1", { project: "p", holder: "alice" }),
    );
    const bob = grant(await reg.claim("e1", { project: "p", holder: "bob" })); // takes it from alice

    // Alice's tab beacons its goodbye with a stale secret; taking the engine token instead would drop BOB's claim while he is driving.
    await reg.release("e1", alice.secret);
    expect(await reg.engineForProject("p")).toBe("e1");
    await reg.release("e1", bob.secret);
    expect(await reg.engineForProject("p")).toBeUndefined();
  });

  // What a server route asks before reaching an engine for an account: a listed engine is not a held one.
  it("says whether an account holds a claim, and forgets it once released", async () => {
    expect(await reg.holds("e1", "alice")).toBe(false);
    const alice = grant(
      await reg.claim("e1", { project: null, holder: "alice" }),
    );
    expect(await reg.holds("e1", "alice")).toBe(true);
    expect(await reg.holds("e1", "bob")).toBe(false);
    expect(await reg.holds("e1", "")).toBe(false);
    expect(await reg.holds("e2", "alice")).toBe(false);
    await reg.release("e1", alice.secret);
    expect(await reg.holds("e1", "alice")).toBe(false);
  });

  it("names the holder and the project to that account, and to nobody else", async () => {
    grant(
      await reg.claim("e1", { project: "band/winter.k7", holder: "alice" }),
    );
    const row = (await reg.list(undefined, "alice")).find((e) => e.id === "e1");
    expect(row?.heldBy).toBe("alice");
    expect(row?.heldProject).toBe("band/winter.k7");
    for (const viewer of ["bob", "", undefined]) {
      const other = (await reg.list(undefined, viewer)).find(
        (e) => e.id === "e1",
      );
      expect(other?.assigned).toBe(true);
      expect(other?.heldBy).toBeUndefined();
      expect(other?.heldProject).toBeUndefined();
      expect(other?.heldProjects).toEqual([]);
    }
    // The addresses stay: a browser probes its own machines directly.
    expect((await reg.list(undefined, "bob"))[0]?.addresses.port).toBe(8765);
  });

  describe("the bench is a session of its own", () => {
    const SONG = "@local/ws/song.k7";
    const BENCH = "@banc/vocals/warm/live.k7";

    it("sits beside its account's song on one engine, and both are listed", async () => {
      const song = grant(
        await reg.claim("e1", { project: SONG, holder: "alice" }),
      );
      const bench = grant(
        await reg.claim("e1", { project: BENCH, holder: "alice" }),
      );
      expect(bench.tookOver).toBeUndefined();
      // The song's tab renews as if nothing happened.
      expect(
        (
          await reg.claim("e1", {
            project: SONG,
            holder: "alice",
            secret: song.secret,
          })
        ).ok,
      ).toBe(true);
      const row = (await reg.list(undefined, "alice")).find(
        (e) => e.id === "e1",
      );
      expect(row?.heldProject).toBe(SONG);
      expect(row?.heldProjects?.map((h) => h.project).sort()).toEqual(
        [BENCH, SONG].sort(),
      );
    });

    it("still gives way to another account, and a second song still takes the first's place", async () => {
      grant(await reg.claim("e1", { project: SONG, holder: "alice" }));
      const bob = grant(
        await reg.claim("e1", { project: BENCH, holder: "bob" }),
      );
      expect(bob.tookOver).toEqual({ holder: "alice", project: SONG });

      const other = grant(
        await reg.claim("e1", { project: "@local/ws/other.k7", holder: "bob" }),
      );
      expect(other.tookOver).toBeUndefined();
      const again = grant(
        await reg.claim("e1", { project: "@local/ws/third.k7", holder: "bob" }),
      );
      expect(again.tookOver).toEqual({
        holder: "bob",
        project: "@local/ws/other.k7",
      });
    });
  });

  /** The three refusals are three different sentences on screen, which is the whole point of `lost` — see docs/claim.md. */
  describe("says WHY the claim is gone", () => {
    it("`project` — the same project, taken by somebody else", async () => {
      const mine = grant(
        await reg.claim("e1", { project: "p", holder: "alice" }),
      );
      grant(await reg.claim("e2", { project: "p", holder: "bob" }));

      const r = await reg.claim("e1", {
        project: "p",
        holder: "alice",
        secret: mine.secret,
      });
      expect(r.ok).toBe(false);
      if (r.ok) return;
      expect(r.lost).toBe("project");
      expect(r.heldBy).toBe("bob");
    });

    it("`engine` — a DIFFERENT project took the machine, and it is named", async () => {
      const mine = grant(
        await reg.claim("e1", { project: "band/winter.k7", holder: "alice" }),
      );
      grant(
        await reg.claim("e1", { project: "band/leclip.k7", holder: "bob" }),
      );

      const r = await reg.claim("e1", {
        project: "band/winter.k7",
        holder: "alice",
        secret: mine.secret,
      });
      expect(r.ok).toBe(false);
      if (r.ok) return;
      // Naming it is what lets the tab offer "pick another engine" instead of a wrong apology.
      expect(r.lost).toBe("engine");
      expect(r.heldBy).toBe("bob");
      expect(r.tookFor).toBe("band/leclip.k7");
    });

    it("`lapsed` — nobody took anything; the claim just aged out", async () => {
      const mine = grant(
        await reg.claim("e1", { project: "p", holder: "alice" }),
      );

      // A sleep, or a network gap longer than the TTL. The engine is still beating.
      await core.clock.advance(ENGINE_ASSIGN_TTL_MS + 1);
      await beat(reg, "e1");

      const r = await reg.claim("e1", {
        project: "p",
        holder: "alice",
        secret: mine.secret,
      });
      // An open would be granted here; a RENEWAL is refused, because the secret is stale.
      expect(r.ok).toBe(false);
      if (r.ok) return;
      expect(r.lost).toBe("lapsed");
      expect(r.heldBy).toBe("");
    });

    it("`lapsed` for a project-LESS holder, however busy the engine has since become", async () => {
      const catalogue = grant(
        await reg.claim("e1", { project: null, holder: "a" }),
      );
      await core.clock.advance(ENGINE_ASSIGN_TTL_MS + 1);
      await beat(reg, "e1");
      grant(await reg.claim("e1", { project: "band/winter.k7", holder: "b" }));

      const r = await reg.claim("e1", {
        project: null,
        holder: "a",
        secret: catalogue.secret,
      });
      expect(r.ok).toBe(false);
      if (r.ok) return;
      // NOT `engine`: pl8 sitting beside a song is legal, so that song took nothing from it and only the TTL did.
      expect(r.lost).toBe("lapsed");
    });

    it("`lapsed` too when the engine itself went away — nobody displaced us", async () => {
      const mine = grant(
        await reg.claim("e1", { project: "p", holder: "alice" }),
      );
      await reg.remove("e1", "tok-e1");

      const r = await reg.claim("e1", {
        project: "p",
        holder: "alice",
        secret: mine.secret,
      });
      expect(r.ok).toBe(false);
      if (r.ok) return;
      expect(r.lost).toBe("lapsed");
    });
  });

  it("keeps holders across a heartbeat — an engine beats every 5s, a claim lives 15", async () => {
    const g = grant(await reg.claim("e1", { project: "p", holder: "alice" }));
    await beat(reg, "e1");
    expect(
      grant(
        await reg.claim("e1", {
          project: "p",
          holder: "alice",
          secret: g.secret,
        }),
      ).secret,
    ).toBe(g.secret);
  });
});

describeCore("the performance rating", () => {
  it("is listed as the heartbeat carried it, and absent when it carried none", async () => {
    const core = await startCore();
    const reg = core.reg;
    const performance = {
      score: 1240,
      cores: 8,
      singleThread: 70,
      diskWriteMBps: 900,
      fsyncPerSec: null,
      memoryGB: 16,
    };
    await reg.upsert({
      id: "rated",
      token: "t",
      hostname: "rated",
      machineFingerprint: "fp",
      addresses: {
        loopback: "127.0.0.1",
        lanIps: [],
        port: 8765,
      },
      performance,
    });
    await beat(reg, "older");
    const byId = new Map((await reg.list()).map((e) => [e.id, e]));
    expect(byId.get("rated")?.performance).toEqual(performance);
    expect(byId.get("older")?.performance).toBeUndefined();
    await core.stop();
  });
});

describeCore("the engine wire's version in the list", () => {
  it("reads a supervisor that states none as protocol 1, and passes on what one states", async () => {
    const core = await startCore();
    const reg = core.reg;
    await beat(reg, "old");
    await reg.upsert({
      id: "new",
      token: "tok-new",
      hostname: "new",
      machineFingerprint: "fp-new",
      addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
      protocol: 1,
      build: "v1.2.3",
      safeMode: true,
    });
    const rows = new Map((await reg.list()).map((e) => [e.id, e]));
    expect(rows.get("old")?.protocol).toBe(1);
    expect(rows.get("old")?.build).toBeUndefined();
    expect(rows.get("old")?.outdated).toBeUndefined();
    expect(rows.get("new")).toMatchObject({
      protocol: 1,
      build: "v1.2.3",
      safeMode: true,
    });
    expect(rows.get("new")?.outdated).toBeUndefined();
    await core.stop();
  });
});
