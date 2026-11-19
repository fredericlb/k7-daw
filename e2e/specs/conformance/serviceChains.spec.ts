// serviceChains.spec.ts — the account's chains read as a client sees them (1p-D18): a file chain with an input and a
// bypassed step, a folder chain with a missing node or a node called IN, a cold chain with a model absent, a live module
// flagged cold, an engine with params, each of the three lists whole for the account's rows and by keys for k7-core's,
// the hub's cut and the download's refusals. A chain that does not parse comes last: it declines every list (1p-D9).
// Run in the service lane: it writes the account's library, which serviceModules reads.
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { expect, test } from "@playwright/test";
import { Service } from "../../src/service.ts";
import { E2E_USER } from "../../src/session.ts";
import { freshWorkspace } from "../../src/sync.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";
const internalSecret = process.env.CONFORMANCE_INTERNAL_SECRET;
const ws = freshWorkspace();
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 120_000 });

const ACCOUNT = "_libraries/account";
const SEP = (url: string) =>
  `cold(true)\nplugin({ type: "offline", name: "sep" })\nparams({ onnx: "conf-sep.onnx" })\n${url}`;
const MODULES: Record<string, string> = {
  "cold/conf-sep": SEP('model_url("https://models.example/conf-sep.onnx")\n'),
  "cold/conf-nourl": SEP(""),
  "cold/conf-inst": `${SEP('model_url("https://models.example/conf-inst.onnx")\n')}outputs([name("instrumental").type("audio"), name("vocals").type("audio")])\n`,
  "audio/conf-gate": 'cold(true)\nplugin({ type: "internal", name: "Gate" })\n',
  "audio/conf-delay": 'engine("tape")\n',
  "audio/conf-delay/engines/tape":
    'plugin({ type: "internal", name: "Delay tape" })\n',
};
const use = (module: string, extra = "") =>
  `steps([id("s1").extends("@account/${module}")${extra}, to([chain("@out")])])\n`;
const CHAINS: Record<string, string> = {
  "conf-mix":
    'input({ channel: [1, 2], monitor: true })\nsteps([id("c1").plugin({ type: "internal", name: "Comp" }).params({ Ratio: 0.5, Mode: "soft" }), id("b1").plugin({ type: "internal", name: "Delay" }).bypass(true), to([chain("@out")])])\n',
  "conf-cold": use("cold/conf-sep"),
  "conf-nourl": use("cold/conf-nourl"),
  "conf-inst": use("cold/conf-inst"),
  "conf-gate": use("audio/conf-gate"),
  "conf-eng": `steps([id("d1").extends("@account/audio/conf-delay").engine("tape").engines({ tape: { params: { Wow: 0.2 } } }), to([chain("@out")])])\n`,
};
const BAD = "conf-bad";
const FOLDER = {
  "conf-fold/graph.k7x":
    'modules({ "@in": ["amp"], amp: ["gone"], gone: ["@out"] })\n',
  "conf-fold/nodes/amp.k7x": 'plugin({ type: "internal", name: "Amp" })\n',
  "conf-in/graph.k7x": 'modules({ "@in": ["IN"], IN: ["@out"] })\n',
  "conf-in/nodes/IN.k7x": 'plugin({ type: "internal", name: "Amp" })\n',
};
// Folders the spec made, deepest first when removed: a library folder it leaves breaks serviceModules' library order.
const made: string[] = [];

async function put(path: string, content: string) {
  const at = path.lastIndexOf("/");
  const dir = path.slice(0, at);
  const parts = dir.split("/");
  for (let i = 1; i < parts.length; i++) {
    const r = await svc.post(
      "/folder",
      { dir: parts.slice(0, i).join("/"), name: parts[i] },
      ws,
    );
    if (r.status === 200) made.push(parts.slice(0, i + 1).join("/"));
  }
  const r = await svc.upload(ws, dir, path.slice(at + 1), Buffer.from(content));
  expect(r.status, `${path}: ${JSON.stringify(r.body)}`).toBe(200);
}

test.beforeAll(async () => {
  test.setTimeout(300_000);
  svc = await Service.connect();
  await svc.create(ws);
  // A run cut short left its files: an upload refuses a file that exists, so they go first.
  for (const path of everything())
    await svc.post("/entry/delete", { path }, ws);
  for (const [name, text] of Object.entries(MODULES))
    await put(`${ACCOUNT}/modules/${name}/module.k7x`, text);
  for (const [name, text] of Object.entries(CHAINS))
    await put(`${ACCOUNT}/chains/${name}.k7x`, text);
  for (const [name, text] of Object.entries(FOLDER))
    await put(`${ACCOUNT}/chains/${name}`, text);
});

/** What the spec writes into the account's library, the folders it made last. */
const everything = () => [
  ...Object.keys(MODULES).map((n) => `${ACCOUNT}/modules/${n}`),
  ...Object.keys(CHAINS).map((n) => `${ACCOUNT}/chains/${n}.k7x`),
  `${ACCOUNT}/chains/${BAD}.k7x`,
  `${ACCOUNT}/chains/conf-fold`,
  `${ACCOUNT}/chains/conf-in`,
];

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  for (const path of everything())
    await svc.post("/entry/delete", { path }, ws);
  for (const path of [...made].reverse())
    await svc.post("/entry/delete", { path }, ws);
  await svc.remove(ws);
  await svc.dispose();
});

type Row = { ref: string } & Record<string, unknown>;
const mine = (rows: Row[]) =>
  rows.filter((r) => r.ref.startsWith("@account/conf-"));

test("the frozen list holds the account's chains whole and the libraries' rows by their keys", async () => {
  const rows: Row[] = (await svc.get("/account/frozen-chains")).body.chains;
  const got: Record<string, Row> = Object.fromEntries(
    mine(rows).map((r) => [r.ref, r]),
  );
  const live = (n: string) => ({ ref: `@account/${n}`, name: n, mine: true });
  expect(got["@account/conf-mix"]).toEqual({
    ...live("conf-mix"),
    liveRefusal: null,
    cold: false,
    steps: [
      { id: "c1", module: "Comp", params: { Ratio: 0.5, Mode: "soft" } },
      { id: "b1", module: "Delay", params: {}, bypass: true },
    ],
    input: { channel: [1, 2], monitor: true },
  });
  expect(got["@account/conf-cold"]).toEqual({
    ...live("conf-cold"),
    liveRefusal: '"@account/cold/conf-sep" only works on audio already on disk',
    cold: true,
    steps: [{ id: "s1", module: "@account/cold/conf-sep", params: {} }],
  });
  // A live module flagged cold is refused live and is no cold chain: the cold half of needsDisk alone.
  expect(got["@account/conf-gate"]).toEqual({
    ...live("conf-gate"),
    liveRefusal:
      '"@account/audio/conf-gate" only works on audio already on disk',
    cold: false,
    steps: [{ id: "s1", module: "@account/audio/conf-gate", params: {} }],
  });
  // The step's engine, with the params its `engines` entry holds, and a module that names the engine itself.
  expect(got["@account/conf-eng"]).toEqual({
    ...live("conf-eng"),
    liveRefusal: null,
    cold: false,
    steps: [
      {
        id: "d1",
        module: "@account/audio/conf-delay",
        engine: "tape",
        params: { Wow: 0.2 },
      },
    ],
  });
  expect(got["@account/conf-fold"]).toMatchObject({
    liveRefusal: "it lacks gone (no nodes/gone.k7x)",
    cold: false,
  });
  expect(got["@account/conf-in"]).toMatchObject({
    liveRefusal: `its node "IN" has the name of one of the rig's own`,
    cold: false,
  });
  // The account's rows come first, each library's after: k7-core's only by their keys.
  const names = rows.map((r) => r.mine);
  expect(names).toEqual([...names].sort((a, b) => Number(b) - Number(a)));
  for (const r of rows.filter((x) => !x.mine))
    expect(Object.keys(r).slice(0, 6)).toEqual([
      "ref",
      "name",
      "mine",
      "liveRefusal",
      "cold",
      "steps",
    ]);
});

test("the live list says why a chain cannot be played, and null for one that can", async () => {
  const rows: Row[] = (await svc.get("/account/live-chains")).body.chains;
  const why = Object.fromEntries(mine(rows).map((r) => [r.ref, r.unavailable]));
  expect(why["@account/conf-mix"]).toBeNull();
  expect(why["@account/conf-cold"]).toBe(
    '"@account/cold/conf-sep" only works on audio already on disk',
  );
  expect(why["@account/conf-fold"]).toBe("it lacks gone (no nodes/gone.k7x)");
  expect(why["@account/conf-in"]).toBe(
    `its node "IN" has the name of one of the rig's own`,
  );
  expect(why["@account/conf-gate"]).toBe(
    '"@account/audio/conf-gate" only works on audio already on disk',
  );
  expect(why["@account/conf-eng"]).toBeNull();
  for (const r of rows) expect(Object.keys(r)).toEqual(["ref", "unavailable"]);
});

test("the cold list holds the chains whose every step is offline, with what each lacks and can download", async () => {
  const body = (await svc.get("/account/cold-chains")).body;
  expect(Object.keys(body)).toEqual([
    "chains",
    "stemsChain",
    "instrumentalChain",
  ]);
  const rows: Row[] = body.chains;
  const got = Object.fromEntries(mine(rows).map((r) => [r.ref, r]));
  const lacks = (m: string) =>
    `step '@account/cold/${m}' wants the model '${"conf-sep.onnx"}', which is not on this machine — put it in ~/.k7/models (or the module's own assets/)`;
  expect(got["@account/conf-cold"]).toEqual({
    ref: "@account/conf-cold",
    instrumental: false,
    unavailable: lacks("conf-sep"),
    downloads: [
      { file: "conf-sep.onnx", url: "https://models.example/conf-sep.onnx" },
    ],
  });
  expect(got["@account/conf-inst"]).toMatchObject({ instrumental: true });
  expect(got["@account/conf-nourl"]).toMatchObject({ downloads: [] });
  // A folder chain with a missing node and a chain that does not parse are no cold chain.
  expect(got["@account/conf-fold"]).toBeUndefined();
  expect(got["@account/conf-gate"]).toBeUndefined();
  expect(got["@account/conf-eng"]).toBeUndefined();
  const refs = rows.map((r) => r.ref);
  expect(refs).toEqual([...refs].sort());
});

test("the download refuses a chain that is not cold and one with nothing to fetch", async () => {
  const none = await svc.post("/account/cold-chains/download", {
    ref: "@nope/x",
  });
  expect([none.status, none.body]).toEqual([
    404,
    { error: "'@nope/x' is not a cold chain of the library" },
  ]);
  const nothing = await svc.post("/account/cold-chains/download", {
    ref: "@account/conf-nourl",
  });
  expect([nothing.status, nothing.body]).toEqual([
    400,
    { error: "'@account/conf-nourl' needs no model this machine can download" },
  ]);
});

/** One POST to the front's internal routes, as the hub asks them. */
function internal(
  body: unknown,
): Promise<{ status: number; body: Record<string, unknown> }> {
  const url = new URL("/api/internal/files/cold-run", base);
  const send = url.protocol === "https:" ? httpsRequest : httpRequest;
  return new Promise((resolve, reject) => {
    const req = send(
      url,
      {
        method: "POST",
        headers: {
          "x-k7-internal": internalSecret ?? "",
          "content-type": "application/json",
        },
      },
      (res) => {
        const parts: Buffer[] = [];
        res.on("data", (c: Buffer) => parts.push(c));
        res.on("end", () => {
          const text = Buffer.concat(parts).toString("utf8");
          resolve({
            status: res.statusCode ?? 0,
            body: text ? JSON.parse(text) : {},
          });
        });
      },
    );
    req.on("error", reject);
    req.end(JSON.stringify(body));
  });
}

test("the hub's cut refuses with the sentences a person can act on", async () => {
  test.skip(!internalSecret, "CONFORMANCE_INTERNAL_SECRET not given");
  const user = E2E_USER.email;
  const asked = async (want: string, chain: string) =>
    await internal({ user, want, chain });
  const bad = await asked("both", "@account/conf-cold");
  expect([bad.status, bad.body.error]).toEqual([
    422,
    "want must be 'stems' or 'instrumental'",
  ]);
  const unset = await asked("stems", "");
  expect(unset.status).toBe(422);
  expect(String(unset.body.error)).toMatch(
    /^no chain named, and this account separates nothing yet — /,
  );
  const flat = await asked("stems", "@account/conf-mix");
  expect(flat.status).toBe(422);
  expect(String(flat.body.error)).toMatch(
    /^'@account\/conf-mix' does not separate — it puts the signal through 1 module\(s\)$/,
  );
  const absent = await asked("stems", "@account/conf-cold");
  expect(absent.status).toBe(422);
  expect(String(absent.body.error)).toBe(
    `chain '@account/conf-cold' has steps this machine cannot load: step '@account/cold/conf-sep' wants the model 'conf-sep.onnx', which is not on this machine — put it in ~/.k7/models (or the module's own assets/)`,
  );
  const mix = await asked("instrumental", "@account/conf-cold");
  expect(mix.status).toBe(422);
});

test("a chain that does not parse is listed unreadable, refused live and no cold chain", async () => {
  const at = `${ACCOUNT}/chains/${BAD}.k7x`;
  await put(at, "steps(");
  try {
    const frozen: Row[] = (await svc.get("/account/frozen-chains")).body.chains;
    const bad = mine(frozen).find((r) => r.ref === `@account/${BAD}`);
    expect(String(bad?.unreadable ?? "")).not.toBe("");
    expect(bad).toMatchObject({ steps: [], cold: false });
    const live: Row[] = (await svc.get("/account/live-chains")).body.chains;
    const why = mine(live).find(
      (r) => r.ref === `@account/${BAD}`,
    )?.unavailable;
    expect(String(why)).toMatch(/^it does not read — /);
    const cold: Row[] = (await svc.get("/account/cold-chains")).body.chains;
    expect(cold.some((r) => r.ref === `@account/${BAD}`)).toBe(false);
  } finally {
    await svc.post("/entry/delete", { path: at }, ws);
  }
});
