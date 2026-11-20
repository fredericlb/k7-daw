// The export hosts: the desktop's front routes and the phone's bridge calls, against fakes of each.

import { afterEach, describe, expect, it, vi } from "vitest";

const calls: { method: string; arg?: Record<string, string> }[] = [];
vi.mock("./settings/androidEngine", () => ({
  onAndroid: true,
  engineCall: async (method: string, arg?: Record<string, string>) => {
    calls.push({ method, arg });
    if (method === "exportBegin") return { token: "t1" };
    if (method === "exportEnd") return { name: "mix (2).wav", where: "uri:1" };
    return {};
  },
}));

import { CHUNK, desktop, phone } from "./exportHost";

afterEach(() => {
  calls.length = 0;
  vi.unstubAllGlobals();
});

describe("the desktop host", () => {
  it("asks the front for the folder and says cancelled as null", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        Response.json({ path: "/Users/me/Music", name: "Music" }),
      )
      .mockResolvedValueOnce(Response.json({ cancelled: true }));
    vi.stubGlobal("fetch", fetchMock);
    expect(await desktop.pickFolder()).toEqual({
      id: "/Users/me/Music",
      path: "/Users/me/Music",
      name: "Music",
    });
    expect(await desktop.pickFolder()).toBeNull();
    expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/local/export/folder");
  });

  it("puts the bytes in the folder under the name it was given", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(
        Response.json({ path: "/m/a (2).wav", name: "a (2).wav" }),
      );
    vi.stubGlobal("fetch", fetchMock);
    const folder = { id: "/m", path: "/m", name: "m" };
    const saved = await desktop.save(folder, "a.wav", new Blob(["x"]));
    expect(saved).toEqual({ folder, name: "a (2).wav", where: "/m/a (2).wav" });
    const [url, init] = fetchMock.mock.calls[0] ?? [];
    expect(String(url)).toBe("/api/local/export/file?folder=%2Fm&name=a.wav");
    expect((init as RequestInit).method).toBe("PUT");
  });

  it("says the front's refusal", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          Response.json(
            { error: "that folder was not chosen" },
            { status: 403 },
          ),
        ),
    );
    await expect(
      desktop.save({ id: "/x", name: "x" }, "a.wav", new Blob(["x"])),
    ).rejects.toThrow("not chosen");
  });
});

describe("the phone host", () => {
  it("opens, sends the bytes in chunks and closes", async () => {
    const folder = { id: "tree", name: "Music" };
    const saved = await phone.save(
      folder,
      "mix.wav",
      new Blob([new Uint8Array(CHUNK + 5)]),
    );
    expect(calls.map((c) => c.method)).toEqual([
      "exportBegin",
      "exportChunk",
      "exportChunk",
      "exportEnd",
    ]);
    expect(calls[0]?.arg).toEqual({ folder: "tree", name: "mix.wav" });
    expect(saved).toEqual({ folder, name: "mix (2).wav", where: "uri:1" });
  });

  it("offers the share sheet and no file manager", () => {
    expect([phone.canShare, phone.canReveal]).toEqual([true, false]);
  });
});
