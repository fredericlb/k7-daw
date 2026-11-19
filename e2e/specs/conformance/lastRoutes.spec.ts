// lastRoutes.spec.ts — the backend's last routes as a client sees them: the built page and its fallback, Express's 404,
// the site page, the `/api` guard's bearer, the device door's refusals and the seam's
// TypeScript half. With CONFORMANCE_UPSTREAM, each answer is compared byte for byte with the backend's (e2e/docs/conformance.md).
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { expect, test } from "@playwright/test";
import { sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";
const upstream = process.env.CONFORMANCE_UPSTREAM;
const internalSecret = process.env.CONFORMANCE_INTERNAL_SECRET;

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
    at?: string;
  } = {},
): Promise<Answer> {
  const url = new URL(path, opts.at ?? base);
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
  expect(page.headers["content-type"]).toBe("text/html; charset=UTF-8");
  expect(page.headers["cache-control"]).toBe("no-cache");
  expect(page.headers["content-security-policy"]).toBe(
    "frame-ancestors 'self'",
  );
  expect(page.headers["x-frame-options"]).toBe("SAMEORIGIN");
  expect(page.headers.vary).toBe("Accept-Encoding");
  expect(page.headers["accept-ranges"]).toBe("bytes");
  expect(page.body.equals(root.body)).toBe(true);
  // `/` is express.static's index, which names no encoding.
  expect(root.headers.vary).toBeUndefined();
  const again = await raw("/conformance/a/page", {
    headers: { "if-none-match": page.headers.etag },
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
  expect(br.headers["content-type"]).toBe(
    "application/javascript; charset=utf-8",
  );
  expect(br.headers["cache-control"]).toBe(
    "public, max-age=31536000, immutable",
  );
  expect(br.headers.vary).toBe("Accept-Encoding");
  expect(br.headers["accept-ranges"]).toBeUndefined();
  const gz = await raw(asset, { headers: { "accept-encoding": "gzip" } });
  expect(gz.headers["content-encoding"]).toBe("gzip");
  const plain = await raw(asset, {
    headers: { "accept-encoding": "br;q=0" },
  });
  expect(plain.headers["content-encoding"]).toBeUndefined();
  expect(plain.headers["content-type"]).toBe(
    "application/javascript; charset=UTF-8",
  );
  expect(plain.headers["accept-ranges"]).toBe("bytes");
  expect(plain.headers.etag).toMatch(/^W\/"[0-9a-f]+-[0-9a-f]+"$/);
  const head = await raw(asset, { method: "HEAD" });
  expect(head.status).toBe(200);
  expect(head.body.length).toBe(0);
  expect(head.headers["content-length"]).toBe(plain.headers["content-length"]);
});

test("a missing asset, /api and /auth are Express's 404, never the shell", async () => {
  test.skip((await raw("/")).status !== 200, "no built page here");
  const asset = await raw("/assets/conformance-missing.js");
  expect(asset.status).toBe(404);
  expect(asset.headers["content-type"]).toBe("text/html; charset=utf-8");
  expect(asset.headers["content-security-policy"]).toBe("default-src 'none'");
  expect(asset.body.toString()).toBe(
    '<!DOCTYPE html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n<title>Error</title>\n</head>\n<body>\n<pre>Cannot GET /assets/conformance-missing.js</pre>\n</body>\n</html>\n',
  );
  const auth = await raw("/auth/conformance-nope?x=<1>");
  expect(auth.status).toBe(404);
  expect(auth.body.toString()).toContain(
    "<pre>Cannot GET /auth/conformance-nope</pre>",
  );
  const post = await raw("/conformance/a/page", { method: "POST" });
  expect(post.status).toBe(404);
  expect(post.body.toString()).toContain("Cannot POST /conformance/a/page");
  const anonymous = await raw("/api/conformance-nope");
  expect(anonymous.status).toBe(401);
  expect(JSON.parse(anonymous.body.toString())).toEqual({
    error: "unauthenticated",
  });
  const signed = await raw("/api/conformance-nope", {
    headers: { cookie: cookie() },
  });
  expect(signed.status).toBe(404);
  expect(signed.body.toString()).toContain(
    "<pre>Cannot GET /api/conformance-nope</pre>",
  );
  const folder = await raw("/assets?x=1");
  expect(folder.status).toBe(301);
  expect(folder.headers.location).toBe("/assets/?x=1");
});

test("a bearer that does not verify is refused even beside a valid session", async () => {
  for (const path of ["/api/preferences", "/api/ai/settings"]) {
    const res = await raw(path, {
      headers: { cookie: cookie(), authorization: "Bearer conformance.forged" },
    });
    expect(res.status, path).toBe(401);
    expect(JSON.parse(res.body.toString()), path).toEqual({
      error: "invalid token",
    });
  }
});

test("the seam's TypeScript half: a bearer's subject and what the backend runs", async () => {
  const refused = await raw("/api/internal/files/verify-token", {
    method: "POST",
  });
  expect(refused.status).toBe(401);
  test.skip(!internalSecret, "CONFORMANCE_INTERNAL_SECRET not given");
  const gate = { "x-k7-internal": internalSecret ?? "" };
  const json = { ...gate, "content-type": "application/json" };
  const forged = await raw("/api/internal/files/verify-token", {
    method: "POST",
    headers: json,
    body: JSON.stringify({ token: "conformance.forged" }),
  });
  expect(forged.status).toBe(200);
  expect(forged.headers["cache-control"]).toBe("no-store");
  expect(JSON.parse(forged.body.toString())).toEqual({ sub: null });
  const empty = await raw("/api/internal/files/verify-token", {
    method: "POST",
    headers: json,
    body: "{}",
  });
  expect(empty.status).toBe(400);
  expect(JSON.parse(empty.body.toString())).toEqual({
    error: "a token is required",
  });
  const none = await raw("/api/internal/files/conformance-nope", {
    headers: gate,
  });
  expect(none.status).toBe(404);
  expect(JSON.parse(none.body.toString())).toEqual({
    error: "no such internal route",
  });
});

test("each answer is the backend's, byte for byte", async () => {
  test.skip(
    !upstream,
    "CONFORMANCE_UPSTREAM names the backend to compare with",
  );
  const asset = await anAsset().catch(() => "/assets/none.js");
  const cases: [string, Record<string, string>, string?][] = [
    ["/", {}],
    ["/", { "accept-encoding": "gzip, deflate, br" }],
    ["/index.html", { "accept-encoding": "br" }],
    ["/index.html", { "accept-encoding": "gzip" }],
    ["/conformance/page?x=1", {}],
    ["/favicon.ico", {}],
    [asset, { "accept-encoding": "br" }],
    [asset, {}],
    [asset, { range: "bytes=10-19" }],
    [asset, { range: "bytes=0-1,4-5" }],
    ["/", { range: "bytes=999999-" }],
    ["/assets/conformance-missing.js", {}],
    ["/assets", {}],
    ["/font", {}],
    ["/.conformance", {}],
    ["/auth/conformance-nope", {}],
    ["/api/conformance-nope", {}],
    ["/api/conformance-nope", { cookie: cookie() }],
    ["/api/conformance-nope", { authorization: "Bearer forged" }],
    ["/", { host: "k7.rocks" }],
    ["/x?y=1", { host: "k7.rocks", accept: "text/html" }],
    ["/x", { host: "www.k7.rocks", accept: "application/json" }],
    ["/favicon.ico", { host: "k7.rocks" }],
    ["/api/internal/files/hub", {}],
  ];
  const compared = [
    "content-type",
    "content-length",
    "content-encoding",
    "content-range",
    "cache-control",
    "etag",
    "last-modified",
    "location",
    "vary",
    "accept-ranges",
    "content-security-policy",
    "x-frame-options",
    "x-content-type-options",
    "referrer-policy",
  ];
  for (const [path, headers, method] of cases) {
    const what = `${method ?? "GET"} ${path} ${JSON.stringify(headers)}`;
    const [mine, theirs] = await Promise.all([
      raw(path, { headers, method }),
      raw(path, { headers, method, at: upstream }),
    ]);
    expect(mine.status, what).toBe(theirs.status);
    expect(mine.body.equals(theirs.body), what).toBe(true);
    for (const h of compared) {
      // Express tags every JSON answer; the front tags none (apps/k7/backend-rs/docs/routes.md).
      if (
        h === "etag" &&
        theirs.headers["content-type"]?.startsWith("application/json")
      )
        continue;
      expect(mine.headers[h], `${what}: ${h}`).toBe(theirs.headers[h]);
    }
  }
});
