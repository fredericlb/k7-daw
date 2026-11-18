// engineJobs.spec.ts — the routes that hand an engine's machine a job (its slots), whichever language serves them:
// their gate, their validation and their refusals, none of which needs an engine.
// With an engine registered, its owner reads its slots once; nothing here changes what an engine plays.
import { expect, test } from "@playwright/test";
import { base, signedIn } from "../../src/instance.ts";

const headers = signedIn();
const nobody = "00000000-0000-4000-8000-000000000000";
const INVALID = "Invalid input: expected string, received undefined";

test("every engine job route is behind the session", async ({ playwright }) => {
  const anonymous = await playwright.request.newContext({ baseURL: base });
  for (const [method, path] of [
    ["GET", `/api/engine-slots/${nobody}/slots`],
    ["POST", `/api/engine-slots/${nobody}/slots`],
  ]) {
    const res = await anonymous.fetch(path, { method });
    expect(res.status(), path).toBe(401);
    expect(await res.json()).toEqual({ error: "unauthenticated" });
  }
  await anonymous.dispose();
});

test("an engine's slots are its owner's: an unknown engine is a 404, a body is checked first", async ({
  request,
}) => {
  const at = `/api/engine-slots/${nobody}`;
  for (const [method, path, data] of [
    ["GET", `${at}/slots`, undefined],
    ["POST", `${at}/slots/s/enable`, undefined],
    ["POST", `${at}/slot/disable`, undefined],
    ["DELETE", `${at}/slots/s`, undefined],
    ["POST", `${at}/slots`, { ref: "@account/x" }],
    ["POST", `${at}/slots/s/footswitch`, { cc: 12 }],
  ] as const) {
    const res = await request.fetch(path, { method, headers, data });
    expect(res.status(), `${method} ${path}`).toBe(404);
    expect(await res.json()).toEqual({
      error: `no engine '${nobody}' is registered`,
    });
  }
  const noRef = await request.post(`${at}/slots`, { headers, data: {} });
  expect(await noRef.json()).toEqual({ error: INVALID });
  const badCc = await request.post(`${at}/slots/s/footswitch`, {
    headers,
    data: { cc: 128 },
  });
  expect(badCc.status()).toBe(400);
  expect(await badCc.json()).toEqual({
    error: "Too big: expected number to be <=127",
  });
});

test("a registered engine's slots are read by its owner and refused to anyone else", async ({
  request,
}) => {
  const engines = (await (
    await request.get("/api/engines", { headers })
  ).json()) as { id: string; ownedBy?: string }[];
  const owned = engines.find((e) => e.ownedBy && e.ownedBy !== "e2e@st0.app");
  test.skip(!owned, "no engine with an owner is registered here");
  const { id, ownedBy } = owned as { id: string; ownedBy: string };
  const stranger = signedIn({
    sub: "conformance",
    email: "conformance@st0.app",
    name: "Conformance",
    groups: [],
  });
  const refused = await request.get(`/api/engine-slots/${id}/slots`, {
    headers: stranger,
  });
  expect(refused.status()).toBe(403);
  expect(await refused.json()).toEqual({
    error: "only the engine's owner sets what it plays",
  });
  // A read: the job asks the machine for its store and changes nothing there.
  const owner = signedIn({
    sub: ownedBy.split("@")[0],
    email: ownedBy,
    name: "Owner",
    groups: [],
  });
  const slots = await request.get(`/api/engine-slots/${id}/slots`, {
    headers: owner,
  });
  test.skip(
    slots.status() === 503,
    "the engine holds no tunnel to this server right now",
  );
  const body = await slots.json();
  expect(Object.keys(body)).toEqual(["slots", "enabledSlot"]);
  expect(Array.isArray(body.slots)).toBe(true);
  expect(
    body.enabledSlot === null || typeof body.enabledSlot === "string",
  ).toBe(true);
  for (const s of body.slots)
    expect(Object.keys(s).sort()).toEqual([
      "chain",
      "footswitch",
      "id",
      "name",
      "savedAt",
    ]);
});
