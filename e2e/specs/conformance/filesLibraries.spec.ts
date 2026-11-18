import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { adminGroup, signedIn } from "../../src/instance.ts";
import { deviceBed } from "../../src/session.ts";
import { FILES_USER, filesRoot } from "../../src/sync.ts";

let ticket = "";
let adminTicket = "";
const libs = "/api/files/account/libraries";
const asUser = () => ({ "x-studio-engine-token": ticket });
const asAdmin = () => ({ "x-studio-engine-token": adminTicket });

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ request }) => {
  const hello = async (groups?: string[]) =>
    (
      await (
        await request.get("/api/files-hub", {
          headers: signedIn(FILES_USER, groups),
        })
      ).json()
    ).ticket;
  ticket = await hello();
  if (adminGroup) adminTicket = await hello([adminGroup]);
});

test("anyone reads the list: each library, whether its folder is here, its published versions and its draft", async ({
  request,
}) => {
  test.skip(
    deviceBed(),
    "a device's libraries come from its origin, not from an admin-written list",
  );
  const res = await request.get(libs, { headers: asUser() });
  expect(res.status()).toBe(200);
  const body = await res.json();
  expect(body.libraries.length).toBeGreaterThan(0);
  for (const l of body.libraries) {
    expect(Object.keys(l)).toEqual(["name", "present", "versions", "draft"]);
    for (const v of l.versions)
      expect(Object.keys(v)).toEqual(
        v.notes ? ["version", "requires", "notes"] : ["version", "requires"],
      );
  }
  if (filesRoot) {
    const listed = JSON.parse(
      readFileSync(join(filesRoot, "libraries.json"), "utf8"),
    ).libraries.map((l: { name: string }) => l.name);
    expect(body.libraries.map((l: { name: string }) => l.name)).toEqual(listed);
    for (const l of body.libraries) {
      const dirs = readdirSync(join(filesRoot, "libraries", l.name))
        .filter((d) => /^\d+$/.test(d))
        .map(Number);
      expect(l.versions.map((v: { version: number }) => v.version)).toEqual(
        dirs.sort((a, b) => a - b),
      );
    }
  }
});

test("only an admin changes the list; one written back as it was answers the list again", async ({
  request,
}) => {
  test.skip(
    deviceBed(),
    "a device's one account is no admin and has no stranger: the libraries' and tests_changes.rs tests hold the refusal",
  );
  const list = (
    await (await request.get(libs, { headers: asUser() })).json()
  ).libraries.map((l: { name: string }) => ({ name: l.name }));
  const refused = await request.put(libs, {
    data: { libraries: list },
    headers: asUser(),
  });
  expect([refused.status(), await refused.json()]).toEqual([
    403,
    { error: "only the origin's admins may change the libraries" },
  ]);
  const publish = await request.post(`${libs}/${list[0].name}/publish`, {
    data: {},
    headers: asUser(),
  });
  expect([publish.status(), await publish.json()]).toEqual([
    403,
    { error: "only the origin's admins may publish a library" },
  ]);
  test.skip(!adminGroup, "env.yaml names no AUTH_ADMIN_GROUPS");
  const before = filesRoot
    ? readFileSync(join(filesRoot, "libraries.json"), "utf8")
    : null;
  const same = await request.put(libs, {
    data: { libraries: list },
    headers: asAdmin(),
  });
  expect(same.status()).toBe(200);
  expect(
    (await same.json()).libraries.map((l: { name: string }) => l.name),
  ).toEqual(list.map((l: { name: string }) => l.name));
  if (filesRoot) {
    const after = readFileSync(join(filesRoot, "libraries.json"), "utf8");
    expect(after).toBe(before);
    expect(after).toBe(`${JSON.stringify({ libraries: list }, null, 2)}\n`);
  }
});

test("an admin's malformed change is refused before anything is written", async ({
  request,
}) => {
  test.skip(!adminGroup, "env.yaml names no AUTH_ADMIN_GROUPS");
  for (const data of [{}, { libraries: "k7-core" }]) {
    const res = await request.put(libs, { data, headers: asAdmin() });
    expect([res.status(), await res.json()]).toEqual([
      400,
      { error: "expected { libraries: [{ name }] }" },
    ]);
  }
  const huge = await request.put(libs, {
    data: Buffer.from(
      `{"libraries":[],"pad":"${"x".repeat(8 * 1024 * 1024)}"}`,
    ),
    headers: { ...asAdmin(), "content-type": "application/json" },
  });
  expect(huge.status()).toBe(413);
  const none = await request.post(`${libs}/conformance-nolib/publish`, {
    data: {},
    headers: asAdmin(),
  });
  expect([none.status(), await none.json()]).toEqual([
    400,
    { error: "'conformance-nolib' has no draft to publish" },
  ]);
  const bad = await request.post(`${libs}/Not%20A%20Lib/publish`, {
    data: {},
    headers: asAdmin(),
  });
  expect([bad.status(), await bad.json()]).toEqual([
    400,
    { error: "'Not A Lib' names no library" },
  ]);
  const withDraft = (
    await (await request.get(libs, { headers: asUser() })).json()
  ).libraries.find((l: { draft: boolean }) => l.draft);
  if (withDraft) {
    const floor = await request.post(`${libs}/${withDraft.name}/publish`, {
      data: { requires: 0 },
      headers: asAdmin(),
    });
    expect(floor.status()).toBe(400);
    expect((await floor.json()).error).toMatch(
      new RegExp(
        `^'${withDraft.name}' cannot require level 0: this k7 runs level \\d+$`,
      ),
    );
  }
  for (const [method, path] of [
    ["DELETE", libs],
    ["GET", `${libs}/k7-core`],
    ["POST", libs],
  ] as const) {
    const res = await request.fetch(path, {
      method,
      headers: asAdmin(),
      data: method === "GET" ? undefined : {},
    });
    expect([method, path, res.status()]).toEqual([method, path, 404]);
  }
});
