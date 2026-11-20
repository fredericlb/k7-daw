import { configureFileApi } from "@studio/project-shell/fileApi";
import {
  ENGINE_TOKEN_HEADER,
  USER_HEADER,
  WORKSPACE_HEADER,
} from "@studio/web-protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { recordPath, service } from "./service";

// The desk reads the account's file service, the one the project is opened through, with its three headers.

const HEADERS = [USER_HEADER, WORKSPACE_HEADER, ENGINE_TOKEN_HEADER];

let sent: { url: string; headers: HeadersInit | undefined }[] = [];

beforeEach(() => {
  configureFileApi({
    base: "http://files.test:8770",
    userEmail: "dj@studio.test",
    workspace: "lemo_mante",
    engineToken: "tok",
    engineId: "e1",
  });
  sent = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      sent.push({ url, headers: init?.headers });
      const body = url.includes("/tree")
        ? [
            {
              path: "records",
              dir: true,
              children: [
                { path: "records/a.flac", dir: false },
                {
                  path: "records/stems",
                  dir: true,
                  children: [{ path: "records/stems/drums.wav", dir: false }],
                },
              ],
            },
          ]
        : { root: "/w/lemo", peak: [], seconds: 1 };
      return {
        ok: true,
        status: 200,
        headers: new Headers({ "content-type": "application/json" }),
        json: async () => body,
        text: async () => JSON.stringify(body),
      } as unknown as Response;
    }),
  );
});
afterEach(() => vi.unstubAllGlobals());

describe("the desk's reads", () => {
  it("asks a record's envelope by its workspace path, with all three headers", async () => {
    await service.envelope("records/a b.flac");
    const last = sent.at(-1);
    expect(last?.url).toBe(
      "http://files.test:8770/record/envelope?path=records%2Fa%20b.flac",
    );
    for (const h of HEADERS) expect(last?.headers).toHaveProperty(h);
  });

  it("lists the workspace's audio files, stems included", async () => {
    expect([...(await service.files())]).toEqual([
      "records/a.flac",
      "records/stems/drums.wav",
    ]);
  });

  it("reads the workspace's root, which a record's path is made from", async () => {
    expect(await service.root()).toBe("/w/lemo");
    expect(recordPath("/w/lemo", "records/a.flac")).toBe(
      "/w/lemo/records/a.flac",
    );
    expect(recordPath("C:\\w\\lemo\\", "records/a.flac")).toBe(
      "C:\\w\\lemo/records/a.flac",
    );
  });
});
