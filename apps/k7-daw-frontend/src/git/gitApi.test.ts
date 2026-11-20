import { afterEach, describe, expect, it, vi } from "vitest";
import { explain, GitError, git } from "./gitApi";

vi.mock("../paraglide/messages.js", () => ({
  m: new Proxy({}, { get: (_t, k) => () => String(k) }),
}));

const t = { base: "/api/files", headers: () => ({ authorization: "x" }) };

const answer = (status: number, body: unknown) =>
  vi.fn().mockResolvedValue(
    new Response(JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json" },
    }),
  );

afterEach(() => vi.unstubAllGlobals());

describe("the git calls", () => {
  it("keeps a refusal's code and what else it said", async () => {
    vi.stubGlobal(
      "fetch",
      answer(409, {
        error: "new host",
        code: "unknown-host",
        fingerprint: "SHA256:x",
      }),
    );
    const e = await git.connect(t, "ws", { url: "git@h:r" }).catch((x) => x);
    expect(e).toBeInstanceOf(GitError);
    expect(e.code).toBe("unknown-host");
    expect(e.body.fingerprint).toBe("SHA256:x");
  });

  it("says pull first for a push behind and commit first for a pull over changes", () => {
    expect(explain(new GitError("x", 409, "behind", {}))).toBe("git.pullFirst");
    expect(explain(new GitError("x", 409, "uncommitted", {}))).toBe(
      "git.commitFirst",
    );
  });

  it("says a host without large files in the host's own words", () => {
    const e = new GitError("h does not store large files", 502, "no-lfs", {});
    expect(explain(e)).toBe("h does not store large files");
  });

  it("asks the next log page by its cursor", async () => {
    const f = answer(200, { commits: [] });
    vi.stubGlobal("fetch", f);
    await git.log(t, "my ws", 30, "abc");
    expect(f.mock.calls[0][0]).toBe(
      "/api/files/git/log?workspace=my%20ws&limit=30&before=abc",
    );
  });

  it("posts the body as json to the route", async () => {
    const f = answer(200, { ok: true });
    vi.stubGlobal("fetch", f);
    await git.resolve(t, "ws", [{ id: "a", choice: "mine" }]);
    const [url, init] = f.mock.calls[0];
    expect(url).toBe("/api/files/git/merge/resolve");
    expect(JSON.parse(init.body)).toEqual({
      workspace: "ws",
      choices: [{ id: "a", choice: "mine" }],
    });
  });
});

describe("the refusals of the review", () => {
  it("says a busy workspace, a waiting merge and a server front in one sentence each", () => {
    expect(explain(new GitError("x", 409, "busy", {}))).toBe("git.busy");
    expect(explain(new GitError("x", 409, "merge-pending", {}))).toBe(
      "git.mergePending",
    );
    expect(explain(new GitError("x", 423, "merge-pending", {}))).toBe(
      "git.mergePending",
    );
    expect(explain(new GitError("x", 403, "device-only", {}))).toBe(
      "git.deviceOnly",
    );
  });

  it("keeps the 409 of a commit over a waiting merge as a GitError", async () => {
    vi.stubGlobal(
      "fetch",
      answer(409, { error: "wait", code: "merge-pending" }),
    );
    const e = await git.commit(t, "ws", "m").catch((x) => x);
    expect(e).toBeInstanceOf(GitError);
    expect(explain(e)).toBe("git.mergePending");
  });
});
