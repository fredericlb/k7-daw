// fakeGithub.ts — a GitHub that answers the device flow, the keys, /meta and the repository list, on 127.0.0.1.
// What a reader gets wrong: like github.com it grants exactly the scopes the device code asked, and approves only when told.
import { createServer, type Server } from "node:http";

export interface FakeGithub {
  /** The device flow's answer turns from "pending" to a token. */
  approve(): void;
  /** The scopes the last device code asked, space-separated as sent. */
  asked(): string;
  keys: string[];
  close(): Promise<void>;
}

/** Listens on `port`, answering with `repos` (name, topics, ssh address) most recently pushed first. */
export async function fakeGithub(
  port: number,
  repos: { name: string; topics: string[]; sshUrl: string }[],
): Promise<FakeGithub> {
  let approved = false;
  let asked = "";
  const keys: string[] = [];
  const reply = (
    res: import("node:http").ServerResponse,
    body: unknown,
    status = 200,
    headers: Record<string, string> = {},
  ) => {
    res.writeHead(status, { "content-type": "application/json", ...headers });
    res.end(JSON.stringify(body));
  };
  const server: Server = createServer((req, res) => {
    const chunks: Buffer[] = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      const path = (req.url ?? "").split("?")[0];
      const route = `${req.method} ${path}`;
      const body = chunks.length
        ? JSON.parse(Buffer.concat(chunks).toString())
        : {};
      if (route === "POST /login/device/code") {
        asked = String(body.scope ?? "");
        return reply(res, {
          device_code: "dc",
          user_code: "ABCD-1234",
          verification_uri: `http://127.0.0.1:${port}/login/device`,
          expires_in: 900,
          interval: 1,
        });
      }
      if (route === "POST /login/oauth/access_token")
        return reply(
          res,
          approved
            ? { access_token: "e2e-token" }
            : { error: "authorization_pending" },
        );
      if (route === "GET /user")
        return reply(res, { login: "octo" }, 200, {
          "x-oauth-scopes": asked.split(" ").filter(Boolean).join(", "),
        });
      if (route === "GET /user/keys")
        return reply(
          res,
          keys.map((key) => ({ key })),
        );
      if (route === "POST /user/keys") {
        keys.push(body.key);
        return reply(res, {}, 201);
      }
      if (route === "GET /meta")
        return reply(res, { ssh_keys: ["ssh-ed25519 aGVsbG8="] });
      if (route === "GET /user/repos")
        return reply(
          res,
          repos.map((r) => ({
            full_name: `octo/${r.name}`,
            ssh_url: r.sshUrl,
            private: true,
            pushed_at: "2026-11-01T00:00:00Z",
            topics: r.topics,
          })),
        );
      reply(res, { message: "Not Found" }, 404);
    });
  });
  await new Promise<void>((ok) => server.listen(port, "127.0.0.1", ok));
  return {
    approve: () => {
      approved = true;
    },
    asked: () => asked,
    keys,
    close: () => new Promise((ok) => server.close(() => ok())),
  };
}
