import { createServer, request } from "node:http";
import type { AddressInfo } from "node:net";
import { peerAddress } from "@studio/web-protocol/services";
import { afterEach, describe, expect, it } from "vitest";
import { trustFront, UNVERIFIED_PEER } from "./front.ts";

const servers: ReturnType<typeof createServer>[] = [];
afterEach(() => {
  for (const s of servers.splice(0)) s.close();
});

/** A server that answers with the peer its handlers would read, and the front headers left on the request. */
async function seen(
  headers: Record<string, string>,
  secret: string | null = "the-secret",
) {
  const server = createServer((req, res) => {
    res.end(
      JSON.stringify({
        peer: peerAddress(req),
        secret: req.headers["x-k7-front"] ?? null,
        header: req.headers["x-k7-peer"] ?? null,
      }),
    );
  });
  servers.push(server);
  trustFront(server, secret ?? undefined);
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  const { port } = server.address() as AddressInfo;
  return new Promise<{ peer: string; secret: null; header: null }>(
    (resolve, reject) => {
      request({ port, host: "127.0.0.1", headers }, (res) => {
        let body = "";
        res.on("data", (c) => {
          body += c;
        });
        res.on("end", () => resolve(JSON.parse(body)));
      })
        .on("error", reject)
        .end();
    },
  );
}

describe("trustFront", () => {
  it("believes the peer the front vouches for", async () => {
    const r = await seen({
      "x-k7-front": "the-secret",
      "x-k7-peer": "203.0.113.9",
    });
    expect(r).toEqual({ peer: "203.0.113.9", secret: null, header: null });
  });

  it("fails closed on a wrong secret, or on a backend given none", async () => {
    const wrong = await seen({
      "x-k7-front": "a-guess",
      "x-k7-peer": "127.0.0.1",
    });
    expect(wrong).toEqual({
      peer: UNVERIFIED_PEER,
      secret: null,
      header: null,
    });
    const forged = await seen({ "x-k7-peer": "127.0.0.1" });
    expect(forged.peer).toBe(UNVERIFIED_PEER);
    const unset = await seen(
      { "x-k7-front": "the-secret", "x-k7-peer": "127.0.0.1" },
      null,
    );
    expect(unset.peer).toBe(UNVERIFIED_PEER);
  });

  it("reads the socket when no front is named", async () => {
    expect((await seen({})).peer).toBe("127.0.0.1");
    expect((await seen({}, null)).peer).toBe("127.0.0.1");
  });
});
