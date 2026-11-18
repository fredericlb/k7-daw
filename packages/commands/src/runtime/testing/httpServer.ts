// httpServer.ts — a FakeServer over HTTP on 127.0.0.1: the routes the runtime's httpRemote calls, for the bundles' own tests.
// What a reader gets wrong: it listens on loopback only and answers the 3d routes' shapes, nothing else.

import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import type { Remote } from "../remote.ts";

const json = (
  res: import("node:http").ServerResponse,
  status: number,
  body: unknown,
): void => {
  const text = JSON.stringify(body);
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Content-Length": Buffer.byteLength(text),
  });
  res.end(text);
};

/** Serves `server` until `close()`; `base` is the URL to give `httpRemote`. */
export async function serveHttp(server: Remote): Promise<{
  base: string;
  close(): Promise<void>;
  bytes: { bundle: number };
}> {
  const bytes = { bundle: 0 };
  const http = createServer(async (req, res) => {
    try {
      const url = new URL(req.url ?? "/", "http://127.0.0.1");
      const q = (k: string): string => url.searchParams.get(k) ?? "";
      if (url.pathname === "/verb-bundle") {
        const b = await server.bundle(q("at"));
        const text = JSON.stringify(b);
        bytes.bundle = Buffer.byteLength(text);
        res.writeHead(200, { "Content-Type": "application/json" });
        return void res.end(text);
      }
      if (url.pathname === "/verb-bundle/library")
        return json(
          res,
          200,
          await server.library(q("at"), q("name"), q("hash")),
        );
      if (url.pathname === "/file") {
        const f = await server.file(q("path"));
        return f === null
          ? json(res, 404, { error: "no such file" })
          : json(res, 200, {
              path: q("path"),
              binary: false,
              content: "text" in f ? f.text : "",
            });
      }
      if (
        url.pathname === "/upload" &&
        req.method === "POST" &&
        server.upload
      ) {
        const chunks: Buffer[] = [];
        for await (const c of req) chunks.push(c as Buffer);
        const a = await server.upload(
          q("dir"),
          q("name"),
          Buffer.concat(chunks),
        );
        return json(res, a.status, a.body);
      }
      if (url.pathname === "/media" && server.bytes) {
        const b = await server.bytes(q("path"));
        if (b === null) return json(res, 404, { error: "no such file" });
        res.writeHead(200, {
          "Content-Type": "application/octet-stream",
          "Content-Length": b.byteLength,
        });
        return void res.end(Buffer.from(b));
      }
      if (
        url.pathname === "/entry/delete" &&
        req.method === "POST" &&
        server.remove
      ) {
        const chunks: Buffer[] = [];
        for await (const c of req) chunks.push(c as Buffer);
        const body = JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
        const a = await server.remove(String(body.path ?? ""));
        return json(res, a.status, a.body);
      }
      if (url.pathname === "/changes" && req.method === "POST") {
        const chunks: Buffer[] = [];
        for await (const c of req) chunks.push(c as Buffer);
        const a = await server.changes(
          JSON.parse(Buffer.concat(chunks).toString("utf8")),
        );
        return json(res, a.status, a.body);
      }
      if (url.pathname === "/verb-host/job") {
        const s = await server.job(q("id"));
        // An unknown or forgotten id is the route's 404.
        return s.state === "failed" && s.code === "gone"
          ? json(res, 404, { error: s.error })
          : json(res, 200, s);
      }
      if (url.pathname === "/waveform") {
        const a = await server.host("waveform", { clip: q("clip") }, "");
        return json(res, a.status, a.body);
      }
      if (url.pathname.startsWith("/verb-host/")) {
        const path = url.pathname.slice("/verb-host/".length);
        // A leading slash after `/verb-host/` is not a route: the bench's save has its own.
        if (path.startsWith("/")) return json(res, 404, { error: "no route" });
        let body: Record<string, unknown> | null = null;
        if (req.method === "POST") {
          const chunks: Buffer[] = [];
          for await (const c of req) chunks.push(c as Buffer);
          body = JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
        }
        const a = await server.host(path, body, url.search.slice(1));
        return json(res, a.status, a.body);
      }
      if (url.pathname === "/bench/save" && req.method === "POST") {
        const chunks: Buffer[] = [];
        for await (const c of req) chunks.push(c as Buffer);
        const body = JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
        const a = await server.host("/bench/save", body, "");
        return json(res, a.status, a.body);
      }
      json(res, 404, { error: "no route" });
    } catch (e) {
      json(res, 500, { error: (e as Error).message });
    }
  });
  http.keepAliveTimeout = 600_000;
  await new Promise<void>((ok) => http.listen(0, "127.0.0.1", ok));
  const port = (http.address() as AddressInfo).port;
  return {
    base: `http://127.0.0.1:${port}`,
    bytes,
    close: () => new Promise<void>((ok) => http.close(() => ok())),
  };
}
