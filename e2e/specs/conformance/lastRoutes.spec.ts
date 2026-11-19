// lastRoutes.spec.ts — the last routes as a client sees them: the built page and its fallback, the 404,
// the `/api` guard (e2e/docs/conformance.md).
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { expect, test } from "@playwright/test";
import { sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";

/** The Cookie header a signed-in e2e account sends. */
function cookie(): string {
  return sessionCookies(base)
    .map((c) => `${c.name}=${c.value}`)
    .join("; ");
}

interface Answer {
  status: number;
  headers: Record<string, string>;
  body: Buffer;
}

/** One request with the Host and headers as given, the body kept as bytes and never decoded. */
function raw(
  path: string,
  opts: {
    method?: string;
    headers?: Record<string, string>;
    body?: string;
  } = {},
): Promise<Answer> {
  const url = new URL(path, base);
  const send = url.protocol === "https:" ? httpsRequest : httpRequest;
  return new Promise((resolve, reject) => {
    const req = send(
      url,
      {
        method: opts.method ?? "GET",
        headers: opts.headers,
        rejectUnauthorized: false,
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => {
          const headers: Record<string, string> = {};
          for (const [k, v] of Object.entries(res.headers))
            if (v !== undefined)
              headers[k] = Array.isArray(v) ? v.join(", ") : v;
          resolve({
            status: res.statusCode ?? 0,
            headers,
            body: Buffer.concat(chunks),
          });
        });
      },
    );
    req.on("error", reject);
    req.end(opts.body);
  });
}

/** A hashed script the shell loads, read from the shell itself. */
async function anAsset(): Promise<string> {
  const shell = (await raw("/")).body.toString();
  const src = /src="(\/assets\/[^"]+\.js)"/.exec(shell)?.[1];
  if (!src) throw new Error("the shell names no script under /assets/");
  return src;
}

test("every page address is the shell, never cached and framed by its own origin only", async () => {
  const root = await raw("/");
  test.skip(
    root.status !== 200,
    "this instance serves no built page (dev: Vite serves it)",
  );
  const page = await raw("/conformance/a/page?x=1");
  expect(page.status).toBe(200);
  expect(page.headers["content-type"]).toMatch(/^text\/html/);
  expect(page.headers["cache-control"]).toBe("no-cache");
  expect(page.headers["content-security-policy"]).toBe(
    "frame-ancestors 'self'",
  );
  expect(page.headers["x-frame-options"]).toBe("SAMEORIGIN");
  expect(page.body.equals(root.body)).toBe(true);
  const again = await raw("/conformance/a/page", {
    headers: { "if-modified-since": page.headers["last-modified"] },
  });
  expect(again.status).toBe(304);
  expect(again.body.length).toBe(0);
  const part = await raw("/", { headers: { range: "bytes=0-9" } });
  expect(part.status).toBe(206);
  expect(part.headers["content-range"]).toBe(`bytes 0-9/${root.body.length}`);
  expect(part.body.equals(root.body.subarray(0, 10))).toBe(true);
});

test("a hashed asset is kept for good, sent compressed when the browser takes it", async () => {
  test.skip((await raw("/")).status !== 200, "no built page here");
  const asset = await anAsset();
  const br = await raw(asset, { headers: { "accept-encoding": "gzip, br" } });
  expect(br.status).toBe(200);
  expect(br.headers["content-encoding"]).toBe("br");
  expect(br.headers["content-type"]).toMatch(/javascript/);
  expect(br.headers["cache-control"]).toBe(
    "public, max-age=31536000, immutable",
  );
  expect(br.headers.vary).toBe("Accept-Encoding");
  const gz = await raw(asset, { headers: { "accept-encoding": "gzip" } });
  expect(gz.headers["content-encoding"]).toBe("gzip");
  const plain = await raw(asset, {
    headers: { "accept-encoding": "identity" },
  });
  expect(plain.headers["content-encoding"]).toBeUndefined();
  expect(plain.headers["content-type"]).toMatch(/javascript/);
  expect(plain.headers["accept-ranges"]).toBe("bytes");
  const head = await raw(asset, { method: "HEAD" });
  expect(head.status).toBe(200);
  expect(head.body.length).toBe(0);
  expect(head.headers["content-length"]).toBe(plain.headers["content-length"]);
});

test("a missing asset, /api and /auth are a 404, never the shell", async () => {
  test.skip((await raw("/")).status !== 200, "no built page here");
  const asset = await raw("/assets/conformance-missing.js");
  expect(asset.status).toBe(404);
  const auth = await raw("/auth/conformance-nope?x=<1>");
  expect(auth.status).toBe(404);
  const post = await raw("/conformance/a/page", { method: "POST" });
  expect(post.status).toBe(404);
  const anonymous = await raw("/api/conformance-nope");
  expect(anonymous.status).toBe(401);
  expect(JSON.parse(anonymous.body.toString())).toEqual({
    error: "unauthenticated",
  });
  const signed = await raw("/api/conformance-nope", {
    headers: { cookie: cookie() },
  });
  expect(signed.status).toBe(404);
});
