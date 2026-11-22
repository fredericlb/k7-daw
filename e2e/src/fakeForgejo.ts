// fakeForgejo.ts — a Forgejo that answers the probe, the authorization-code sign-in with PKCE, keys and repositories, on 127.0.0.1.
// What a reader gets wrong: it redirects the browser to the front's callback by itself, as the real server does once allowed.
import { createHash } from "node:crypto";
import { createServer, type Server, type ServerResponse } from "node:http";

export interface FakeForgejo {
  /** The address the person types: where the fake listens. */
  url: string;
  /** The redirect URIs the authorize page was asked with, the last first. */
  redirects(): string[];
  keys: string[];
  close(): Promise<void>;
}

const CLIENT_ID = "e2e-forgejo";
const LOGIN = "fred";

/** Listens on `port`, listing `repos` (name, topics, ssh address) for the signed-in account. */
export async function fakeForgejo(
  port: number,
  repos: { name: string; topics: string[]; sshUrl: string }[],
): Promise<FakeForgejo & { clientId: string }> {
  const keys: string[] = [];
  const redirects: string[] = [];
  const codes = new Map<string, { challenge: string; redirect: string }>();
  const access = new Set<string>();
  let n = 0;
  const reply = (res: ServerResponse, body: unknown, status = 200) => {
    res.writeHead(status, { "content-type": "application/json" });
    res.end(JSON.stringify(body));
  };
  const registered = (uri: string) => {
    try {
      const u = new URL(uri);
      return (
        u.protocol === "http:" &&
        u.hostname === "127.0.0.1" &&
        u.pathname === "/api/files/git/forge/callback" &&
        !u.search
      );
    } catch {
      return false;
    }
  };
  const server: Server = createServer((req, res) => {
    const chunks: Buffer[] = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      const url = new URL(req.url ?? "/", `http://127.0.0.1:${port}`);
      const route = `${req.method} ${url.pathname}`;
      const text = Buffer.concat(chunks).toString();
      const body: Record<string, string> = text
        ? text.startsWith("{")
          ? JSON.parse(text)
          : Object.fromEntries(new URLSearchParams(text))
        : {};
      if (route === "GET /api/v1/version")
        return reply(res, { version: "17.0.2+gitea-1.22.0" });
      if (route === "GET /api/v1/settings/api")
        return reply(res, { max_response_items: 50, default_paging_num: 30 });
      if (route === "GET /api/v1/settings/repository")
        return reply(res, { lfs_disabled: false, http_git_disabled: false });
      if (route === "GET /.well-known/openid-configuration")
        return reply(res, { issuer: `http://127.0.0.1:${port}/` });
      if (route === "GET /login/oauth/authorize") {
        const q = url.searchParams;
        const redirect = q.get("redirect_uri") ?? "";
        if (q.get("client_id") !== CLIENT_ID || !registered(redirect))
          return reply(res, { message: "Unregistered Redirect URI" }, 400);
        redirects.unshift(redirect);
        n += 1;
        const code = `code-${n}`;
        codes.set(code, {
          challenge: q.get("code_challenge") ?? "",
          redirect,
        });
        const back = new URL(redirect);
        back.searchParams.set("code", code);
        back.searchParams.set("state", q.get("state") ?? "");
        res.writeHead(302, { location: back.toString() });
        return res.end();
      }
      if (route === "POST /login/oauth/access_token") {
        const wanted = codes.get(body.code ?? "");
        codes.delete(body.code ?? "");
        const hashed = createHash("sha256")
          .update(body.code_verifier ?? "")
          .digest("base64url");
        if (
          body.grant_type !== "authorization_code" ||
          !wanted ||
          wanted.redirect !== body.redirect_uri ||
          wanted.challenge !== hashed
        )
          return reply(res, { error: "invalid_grant" }, 400);
        n += 1;
        access.add(`at-${n}`);
        return reply(res, {
          access_token: `at-${n}`,
          token_type: "bearer",
          expires_in: 3600,
          refresh_token: `rt-${n}`,
        });
      }
      const bearer = (req.headers.authorization ?? "").replace("Bearer ", "");
      if (!access.has(bearer))
        return reply(res, { message: "invalid token" }, 401);
      if (route === "GET /api/v1/user")
        return reply(res, { id: 1, login: LOGIN });
      if (route === "GET /api/v1/user/keys") return reply(res, []);
      if (route === "POST /api/v1/user/keys") {
        keys.push(body.key);
        return reply(res, { id: keys.length, title: body.title }, 201);
      }
      if (route === "GET /api/v1/user/repos")
        return reply(
          res,
          url.searchParams.get("page") && url.searchParams.get("page") !== "1"
            ? []
            : repos.map((r) => ({
                full_name: `${LOGIN}/${r.name}`,
                ssh_url: r.sshUrl,
                private: true,
                topics: r.topics,
                updated_at: "2026-11-01T00:00:00Z",
              })),
        );
      if (req.method === "PUT" && url.pathname.includes("/topics/")) {
        res.writeHead(204);
        return res.end();
      }
      reply(res, { message: "Not Found" }, 404);
    });
  });
  await new Promise<void>((ok) => server.listen(port, "127.0.0.1", ok));
  return {
    url: `http://127.0.0.1:${port}`,
    clientId: CLIENT_ID,
    redirects: () => [...redirects],
    keys,
    close: () => new Promise((ok) => server.close(() => ok())),
  };
}
