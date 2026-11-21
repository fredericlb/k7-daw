// flows.spec.ts — what the app does natively and the host's WebKit never reaches, driven through TestFlows.swift's hooks: a download, a private CA, a killed page, the microphone.
// ios target only; each hook runs the app's own code and each result is read from its `[host] flow:` line in the machine log, never from a page.
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createServer, type Server } from "node:https";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { FileService } from "../../src/files.ts";
import { IOS_BUNDLE, simctl, simulator } from "../../src/ios.ts";
import { expect, TARGET, test } from "../../src/target.ts";

const HOOK = "rocks.k7.daw.test.flow.";
const udid = () => simulator();
const container = () => simctl("get_app_container", udid(), IOS_BUNDLE, "data");

/** What the run tells a hook, as a file in the app's tmp folder. */
function say(name: string, text: string): void {
  const dir = join(container(), "tmp");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `k7-flow-${name}`), text);
}

function hook(name: string): void {
  simctl("spawn", udid(), "notifyutil", "-p", HOOK + name);
}

type K7Files = FileService;

/** The phone's engine bound to the file service, which the machine log is read through. */
async function bound(k7: { files: () => Promise<FileService> }) {
  const files = await k7.files();
  const [engine] = (await (await files.request.get("/api/engines")).json()) as {
    id: string;
  }[];
  files.bindEngine(engine?.id ?? "");
  return files;
}

/** When the test began, in the local time `log show --start` reads. */
let began = "";
const stamp = (d: Date) => {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
};

/** The first app log line since the test began that matches `re`, waited for in the simulator's system log. */
async function logged(re: RegExp, wait = 30_000) {
  let found = "";
  await expect
    .poll(
      () => {
        const out = simctl(
          "spawn",
          udid(),
          "log",
          "show",
          "--start",
          began,
          "--info",
          "--style",
          "compact",
          "--predicate",
          'subsystem == "rocks.k7.daw" AND category == "host"',
        );
        found = out.split("\n").findLast((l) => re.test(l)) ?? "";
        return Promise.resolve(found);
      },
      { timeout: wait, intervals: [1000], message: `no log line ${re}` },
    )
    .not.toBe("");
  return found;
}

/** The app relaunched, and its studio answering for the phone's engine. */
async function relaunch(files: K7Files): Promise<void> {
  try {
    simctl("terminate", udid(), IOS_BUNDLE);
  } catch {
    // Already gone.
  }
  simctl("launch", udid(), IOS_BUNDLE);
  await expect
    .poll(
      async () => {
        const res = await files.request.get("/api/engines").catch(() => null);
        return res?.ok() ? ((await res.json()) as unknown[]).length : 0;
      },
      {
        timeout: 120_000,
        intervals: [2000],
        message: "the studio never came back",
      },
    )
    .toBeGreaterThan(0);
  files.bindEngine(
    (
      (await (await files.request.get("/api/engines")).json()) as {
        id: string;
      }[]
    )[0]?.id ?? "",
  );
}

test.beforeEach(() => {
  began = stamp(new Date(Date.now() - 1000));
  test.skip(TARGET !== "ios", "the flows are the app's own, on the simulator");
  test.skip(
    process.env.E2E_IOS_ATTACH === "1",
    "a phone has no simctl: its lines are the checklist's (ios.md)",
  );
  test.setTimeout(240_000);
});

test("a download lands in Documents/Downloads", async ({ k7 }) => {
  // The fixture launches the app, which the hooks live in.
  expect(k7.target).toBe("ios");
  const name = `flow-take-${Date.now()}.wav`;
  say("download", name);
  hook("download");
  await logged(new RegExp(`download: saved ${name}`));
  const saved = join(container(), "Documents", "Downloads", name);
  expect(existsSync(saved)).toBe(true);
  expect(readFileSync(saved, "utf8")).toBe("k7 flow download");
});

test.describe("a private CA", () => {
  const dir = mkdtempSync(join(tmpdir(), "k7-flow-ca-"));
  let server: Server;
  let port = 0;

  const openssl = (...args: string[]) =>
    execFileSync("openssl", args, { cwd: dir, stdio: "pipe" });

  test.beforeAll(async () => {
    writeFileSync(
      join(dir, "ca.cnf"),
      "[req]\ndistinguished_name=dn\nx509_extensions=v3\nprompt=no\n[dn]\nCN=k7 flow CA\n[v3]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n",
    );
    writeFileSync(
      join(dir, "leaf.ext"),
      "basicConstraints=CA:FALSE\nsubjectAltName=IP:127.0.0.1\nextendedKeyUsage=serverAuth\nkeyUsage=digitalSignature\n",
    );
    openssl(
      "ecparam",
      "-name",
      "prime256v1",
      "-genkey",
      "-noout",
      "-out",
      "ca.key",
    );
    openssl(
      "req",
      "-x509",
      "-new",
      "-key",
      "ca.key",
      "-sha256",
      "-days",
      "30",
      "-config",
      "ca.cnf",
      "-out",
      "ca.pem",
    );
    openssl(
      "ecparam",
      "-name",
      "prime256v1",
      "-genkey",
      "-noout",
      "-out",
      "leaf.key",
    );
    openssl(
      "req",
      "-new",
      "-key",
      "leaf.key",
      "-subj",
      "/CN=127.0.0.1",
      "-out",
      "leaf.csr",
    );
    openssl(
      "x509",
      "-req",
      "-in",
      "leaf.csr",
      "-CA",
      "ca.pem",
      "-CAkey",
      "ca.key",
      "-CAcreateserial",
      "-sha256",
      "-days",
      "30",
      "-extfile",
      "leaf.ext",
      "-out",
      "leaf.pem",
    );
    server = createServer(
      {
        key: readFileSync(join(dir, "leaf.key")),
        cert: readFileSync(join(dir, "leaf.pem")),
      },
      (_req, res) => res.end("k7-flow-ok"),
    );
    await new Promise<void>((ok) => server.listen(0, "127.0.0.1", ok));
    port = (server.address() as AddressInfo).port;
  });

  test.afterAll(() => {
    server?.close();
    rmSync(dir, { recursive: true, force: true });
  });

  test("is refused before the import, trusted after it, and a wrong name stays refused", async ({
    k7,
  }) => {
    expect(k7.target).toBe("ios");
    const open = async (url: string, shown: boolean) => {
      say("url", url);
      hook("open");
      const line = await logged(
        new RegExp(`flow: open ${url.replace(/[?.]/g, "\\$&")} `),
      );
      expect(line).toContain(shown ? " shown " : "not shown");
      return line;
    };
    hook("forget-ca");
    await logged(/flow: forgot the imported CAs/);
    await open(`https://127.0.0.1:${port}/?n=1`, false);
    expect(await logged(/trust: 127\.0\.0\.1 refused/)).toBeTruthy();
    say("ca", join(dir, "ca.pem"));
    hook("import-ca");
    await logged(/flow: imported CA k7 flow CA, 1 trusted/);
    expect(await open(`https://127.0.0.1:${port}/?n=2`, true)).toContain(
      "body=k7-flow-ok",
    );
    await logged(/trust: 127\.0\.0\.1 vouched for by an imported CA/);
    await open(`https://localhost:${port}/?n=3`, false);
    await logged(/trust: localhost refused/);
    hook("forget-ca");
  });
});

test("a killed page reloads, twice a minute at most", async ({ k7 }) => {
  const files = await bound(k7);
  await relaunch(files);
  // The first hook reaches a view the shell has shown; give it its studio before killing.
  say("download", `flow-warm-${Date.now()}.wav`);
  hook("download");
  await logged(/download: saved flow-warm-/);
  for (const n of [1, 2]) {
    hook("kill");
    await logged(
      new RegExp(`content process of k7 lost: reload ${n} of 2`),
      30_000,
    );
    await new Promise((r) => setTimeout(r, 6000));
  }
  hook("kill");
  await logged(/content process of k7 lost a third time in a minute/, 30_000);
});

test.describe("the microphone", () => {
  // A privacy change ends the running app, so the grant is restored only where the test left it revoked.
  let revoked = false;
  test.afterAll(() => {
    if (!revoked) return;
    try {
      simctl("privacy", udid(), "grant", "microphone", IOS_BUNDLE);
      simctl("launch", udid(), IOS_BUNDLE);
    } catch {
      // Nothing to restore when the run never got that far.
    }
  });

  test("refused by iOS, then the Settings offer", async ({ k7 }) => {
    const files = await bound(k7);
    try {
      simctl("terminate", udid(), IOS_BUNDLE);
    } catch {
      // Already gone.
    }
    simctl("privacy", udid(), "revoke", "microphone", IOS_BUNDLE);
    revoked = true;
    await relaunch(files);
    hook("mic");
    await logged(/flow: microphone getUserMedia refused/);
    await logged(/microphone: refused by iOS, offering Settings/);
    const dialog = await logged(/flow: microphone dialog/);
    expect(dialog).toContain('"Microphone"');
    expect(dialog).toContain("Not now|Settings");
    simctl("privacy", udid(), "grant", "microphone", IOS_BUNDLE);
    revoked = false;
    await relaunch(files);
  });
});
