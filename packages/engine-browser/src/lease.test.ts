import { ENGINE_ASSIGN_RENEW_MS } from "@studio/web-protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { holdEngine } from "./lease";
import { ClaimLost } from "./registry";

// The loop listens for `pagehide`, and node has no window, so a two-method stub is the whole DOM this needs.
const listeners: Record<string, () => void> = {};
beforeEach(() => {
  vi.useFakeTimers();
  (globalThis as { window?: unknown }).window = {
    addEventListener: (k: string, f: () => void) => {
      listeners[k] = f;
    },
    removeEventListener: (k: string) => {
      delete listeners[k];
    },
  };
});
afterEach(() => {
  vi.useRealTimers();
});

/** Records every select, and answers whatever the queue says — a grant unless told otherwise. */
function api(answers: Array<Error | string> = []) {
  const calls: Array<{ project: string | null; secret?: string }> = [];
  let n = 0;
  return {
    calls,
    released: [] as string[],
    select(
      _id: string,
      _token: string,
      _ms: number,
      project: string | null,
      secret?: string,
    ) {
      calls.push({ project, secret });
      const a = answers[n++] ?? `s${n}`;
      return a instanceof Error
        ? Promise.reject(a)
        : Promise.resolve({ ok: true as const, secret: a });
    },
    release(_id: string, secret: string) {
      this.released.push(secret);
    },
  };
}

describe("holdEngine", () => {
  it("renews with the secret it was granted", async () => {
    const a = api();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first" });
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS + 1);
    expect(a.calls).toEqual([
      { project: "songs/first", secret: undefined },
      { project: "songs/first", secret: "s1" },
    ]);
  });

  it("re-OPENS after a lapse, without telling the surface", async () => {
    const a = api([
      "s1",
      new ClaimLost("", "lapsed"), // asleep long enough for the secret to age out
    ]);
    const onLost = vi.fn();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first", onLost });
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS * 2 + 1);
    expect(onLost).not.toHaveBeenCalled();
    // Third call carries NO secret: an open, which the registry always grants.
    expect(a.calls[2]).toEqual({ project: "songs/first", secret: undefined });
  });

  it("waits a whole tick before re-opening, so a gone engine is not hammered", async () => {
    const a = api([new ClaimLost("", "lapsed"), new ClaimLost("", "lapsed")]);
    holdEngine(a, "e1", "tok", 3, { project: "songs/first" });
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS - 1);
    expect(a.calls).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(2);
    expect(a.calls).toHaveLength(2);
  });

  it("stops for good when somebody took the project", async () => {
    const a = api(["s1", new ClaimLost("ana@x", "project")]);
    const onLost = vi.fn();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first", onLost });
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS * 3 + 1);
    expect(onLost).toHaveBeenCalledWith("ana@x", {
      reason: "project",
      tookFor: null,
    });
    expect(a.calls).toHaveLength(2); // no third: the interval is cleared
  });

  it("stops for good when another project took the engine, naming it", async () => {
    const a = api(["s1", new ClaimLost("ana@x", "engine", "songs/other")]);
    const onLost = vi.fn();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first", onLost });
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS * 2 + 1);
    expect(onLost).toHaveBeenCalledWith("ana@x", {
      reason: "engine",
      tookFor: "songs/other",
    });
  });

  it("stops for good, without retrying, when the engine is another account's", async () => {
    const a = api([new ClaimLost("", "refused")]);
    const onLost = vi.fn();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first", onLost });
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS * 2 + 1);
    expect(onLost).toHaveBeenCalledWith("", {
      reason: "refused",
      tookFor: null,
    });
    expect(a.calls).toHaveLength(1);
  });

  it("keeps renewing through a network miss — only a 409 is an answer", async () => {
    const a = api(["s1", new Error("offline")]);
    const onLost = vi.fn();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first", onLost });
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS * 2 + 1);
    expect(onLost).not.toHaveBeenCalled();
    // Still the secret we hold: a miss loses nothing.
    expect(a.calls[2]).toEqual({ project: "songs/first", secret: "s1" });
  });
});

describe("holdEngine's goodbye", () => {
  const doc: Record<string, () => void> = {};
  let visibility = "visible";
  beforeEach(() => {
    visibility = "visible";
    (globalThis as { document?: unknown }).document = {
      get visibilityState() {
        return visibility;
      },
      addEventListener: (k: string, f: () => void) => {
        doc[k] = f;
      },
      removeEventListener: (k: string) => {
        delete doc[k];
      },
    };
  });
  afterEach(() => {
    delete (globalThis as { document?: unknown }).document;
  });

  it("releases the secret it holds when disposed, and asks nothing after", async () => {
    const a = api();
    const bye = holdEngine(a, "e1", "tok", 3, { project: "songs/first" });
    await vi.advanceTimersByTimeAsync(0);
    bye();
    expect(a.released).toEqual(["s1"]);
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS * 3);
    expect(a.calls).toHaveLength(1);
    expect(listeners.pagehide).toBeUndefined();
    expect(doc.visibilitychange).toBeUndefined();
  });

  it("releases on pagehide, the page's last word", async () => {
    const a = api();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first" });
    await vi.advanceTimersByTimeAsync(0);
    listeners.pagehide?.();
    expect(a.released).toEqual(["s1"]);
  });

  it("renews at once when the tab comes back, and not while it is hidden", async () => {
    const a = api();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first" });
    await vi.advanceTimersByTimeAsync(0);
    visibility = "hidden";
    doc.visibilitychange?.();
    expect(a.calls).toHaveLength(1);
    visibility = "visible";
    doc.visibilitychange?.();
    expect(a.calls).toEqual([
      { project: "songs/first", secret: undefined },
      { project: "songs/first", secret: "s1" },
    ]);
  });

  it("calls onLost once, and renews nothing after it, even when the tab comes back", async () => {
    const a = api([new ClaimLost("ana@x", "project")]);
    const onLost = vi.fn();
    holdEngine(a, "e1", "tok", 3, { project: "songs/first", onLost });
    await vi.advanceTimersByTimeAsync(0);
    doc.visibilitychange?.();
    await vi.advanceTimersByTimeAsync(ENGINE_ASSIGN_RENEW_MS * 2);
    expect(onLost).toHaveBeenCalledTimes(1);
    expect(a.calls).toHaveLength(1);
  });

  it("releases a grant that lands after the goodbye", async () => {
    let grant: (s: { ok: true; secret: string }) => void = () => {};
    const released: string[] = [];
    const late = {
      select: () =>
        new Promise<{ ok: true; secret: string }>((r) => {
          grant = r;
        }),
      release: (_id: string, secret: string) => {
        released.push(secret);
      },
    };
    const bye = holdEngine(late, "e1", "tok", 3, { project: "songs/first" });
    bye();
    grant({ ok: true, secret: "late" });
    await vi.advanceTimersByTimeAsync(0);
    expect(released).toEqual(["late"]);
  });
});
