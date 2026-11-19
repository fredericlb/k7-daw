// ios.ts — the suite's `ios` target: the app on the iOS simulator, its studio driven from the host in WebKit.
// WKWebView has no DevTools socket Playwright reaches, so the page is the host's own WebKit on the app's loopback (docs/e2e.md).
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import {
  type APIRequestContext,
  type BrowserContext,
  test as base,
  devices,
  expect,
  type Page,
  request,
  webkit,
} from "@playwright/test";
import { FileService } from "./files.ts";
import { seedFixtures } from "./fixtures.ts";
import type { K7 } from "./target.ts";

export const IOS_BUNDLE = "se.frediver.k7.daw";

/** The app's port for a test run, handed to it as K7_STUDIO_PORT so the tray's stays free. */
const STUDIO_PORT = 18799;
export const IOS_ORIGIN = `http://127.0.0.1:${STUDIO_PORT}`;

/** A first launch seeds the library before anything answers. */
const BOOT_MS = 180_000;

/** The app's engine port in a test run: 8765 is the user's own engine, and the simulator shares the Mac's loopback. */
const ENGINE_PORT = 18765;

/** `E2E_IOS_ATTACH=1`: the app is already running on a phone behind a USB port forward (engine/k7-ios/scripts/device.sh). */
const ATTACH = process.env.E2E_IOS_ATTACH === "1";

const DERIVED = join(homedir(), ".cache", "k7-ios-dd");

/** The built app: `E2E_IOS_APP`, else the one the staging build leaves in its derived data. */
function appPath(): string {
  const app =
    process.env.E2E_IOS_APP ??
    join(DERIVED, "Build/Products/Debug-iphonesimulator/k7.app");
  if (!existsSync(app)) throw new Error(`no app at ${app}: build it first`);
  return app;
}

/** An iOS runtime's version as a number to sort by: `…SimRuntime.iOS-27-2` is 27.02, anything else 0. */
function runtimeVersion(runtime: string): number {
  const m = /SimRuntime\.iOS-(\d+)-(\d+)/.exec(runtime);
  return m ? Number(m[1]) + Number(m[2]) / 100 : 0;
}

/** The simulator the run uses: `E2E_IOS_DEVICE`, else the booted one on the newest iOS, which enforces what a phone will. */
export function simulator(): string {
  if (process.env.E2E_IOS_DEVICE) return process.env.E2E_IOS_DEVICE;
  const out = execFileSync(
    "xcrun",
    ["simctl", "list", "devices", "booted", "-j"],
    { encoding: "utf8" },
  );
  const booted = Object.entries(
    JSON.parse(out).devices as Record<string, { udid: string }[]>,
  )
    .sort(([a], [b]) => runtimeVersion(b) - runtimeVersion(a))
    .flatMap(([, devices]) => devices);
  if (!booted[0])
    throw new Error("no booted simulator: boot one or set E2E_IOS_DEVICE");
  return booted[0].udid;
}

export function simctl(...args: string[]): string {
  return execFileSync("xcrun", ["simctl", ...args], {
    encoding: "utf8",
    env: {
      ...process.env,
      SIMCTL_CHILD_K7_STUDIO_PORT: String(STUDIO_PORT),
      // Debug builds only: the hooks that drive what the host's WebKit cannot (specs/app/flows.spec.ts).
      SIMCTL_CHILD_K7_TEST_HOOKS: "1",
    },
  }).trim();
}

/** What stops the phone's studio from being usable, or "up". */
async function studioState(api: APIRequestContext): Promise<string> {
  const me = await api.get("/auth/me").catch(() => null);
  if (!me) return "the front does not answer yet";
  if (me.status() !== 200) return `/auth/me answered ${me.status()}`;
  const res = await api.get("/api/engines");
  if (!res.ok()) return `/api/engines answered ${res.status()}`;
  const engines = (await res.json()) as unknown[];
  return engines.length > 0 ? "up" : "no engine listed yet";
}

/** Remembers the phone's engine for the account, as a first pick does (apps/k7/frontend/docs/shell.md). */
async function rememberEngine(api: APIRequestContext): Promise<void> {
  const [engine] = (await (await api.get("/api/engines")).json()) as {
    id: string;
  }[];
  const prefs = (await (await api.get("/api/preferences")).json()) as object;
  const res = await api.put("/api/preferences", {
    data: { ...prefs, engineId: engine?.id },
  });
  if (!res.ok())
    throw new Error(`preferences: ${res.status()} ${await res.text()}`);
}

/** `engine-host.json`'s port set before launch: no SIMCTL_CHILD variable moves it, and the supervisor reads it once. */
function pinEnginePort(udid: string): void {
  const dir = join(
    simctl("get_app_container", udid, IOS_BUNDLE, "data"),
    "Library",
    "Application Support",
    ".k7",
  );
  mkdirSync(dir, { recursive: true });
  const file = join(dir, "engine-host.json");
  let settings: Record<string, unknown> = {};
  try {
    settings = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    // A first launch has no settings yet.
  }
  writeFileSync(
    file,
    JSON.stringify({ ...settings, port: ENGINE_PORT }, null, 2),
  );
}

/** `E2E_STEM_MODEL`'s file cloned into the supervisor's copies of server folders before launch (a cut reads nothing outside them), named to the front as $STUDIO_MODELS. */
function placeStemModel(udid: string): void {
  const model = process.env.E2E_STEM_MODEL;
  if (!model) return;
  const dir = join(
    simctl("get_app_container", udid, IOS_BUNDLE, "data"),
    "Library",
    "Application Support",
    ".k7",
    "remote",
    "models",
  );
  mkdirSync(dir, { recursive: true });
  const to = join(dir, "htdemucs_fp16weights.onnx");
  if (!existsSync(to)) execFileSync("cp", ["-c", model, to]);
  // The front names the models folder from $STUDIO_MODELS, which simctl hands the app on launch.
  process.env.SIMCTL_CHILD_STUDIO_MODELS = dir;
}

interface Phone {
  view: Page;
  context: BrowserContext;
  request: APIRequestContext;
}

export const ios = base.extend<{ k7: K7 }, { phone: Phone }>({
  phone: [
    // biome-ignore lint/correctness/noEmptyPattern: Playwright refuses a fixture whose fixtures argument is not destructured.
    async ({}, use) => {
      // A phone is installed and launched by device.sh, which forwards its port here.
      const udid = ATTACH ? "" : simulator();
      if (!ATTACH) {
        // Relaunched, since the port is read once, when the process starts.
        try {
          simctl("terminate", udid, IOS_BUNDLE);
        } catch {
          // An app that crashed under the last spec is already gone.
        }
        simctl("install", udid, appPath());
        pinEnginePort(udid);
        placeStemModel(udid);
        simctl("launch", udid, IOS_BUNDLE);
      }
      const api = await request.newContext({ baseURL: IOS_ORIGIN });
      await expect
        .poll(
          async () => {
            return studioState(api);
          },
          {
            message: "the app's studio never came up",
            timeout: BOOT_MS,
            intervals: [2000],
          },
        )
        .toBe("up");
      await rememberEngine(api);
      await seedFixtures(await FileService.connect(api, IOS_ORIGIN));
      const browser = await webkit.launch();
      const context = await browser.newContext({
        ...devices["iPhone 15"],
        // Mobile WebKit has no mouse wheel, which the specs use to zoom; the phone's size and agent stay.
        isMobile: false,
        hasTouch: false,
        baseURL: IOS_ORIGIN,
      });
      const view = await context.newPage();
      // The page asks `/auth/me` for its token itself.
      await view.goto(`${IOS_ORIGIN}/`);
      await view.waitForLoadState("networkidle");
      await use({ view, context, request: api });
      await browser.close();
      await api.dispose();
      // The app's engine listens on the loopback until it ends, which a run must not leave behind.
      if (!ATTACH) {
        try {
          simctl("terminate", udid, IOS_BUNDLE);
        } catch {
          // Already gone.
        }
      }
    },
    { scope: "worker", timeout: 2 * BOOT_MS },
  ],
  k7: async ({ phone }, use) => {
    await use({
      target: "ios",
      async open(path) {
        await phone.view.goto(new URL(path, IOS_ORIGIN).href);
        // WebKit reports a fetch a reload cuts short as a page error, which Chromium does not.
        await phone.view.waitForLoadState("networkidle");
        return phone.view;
      },
      async landed(from, at) {
        await expect(from).toHaveURL(at);
        return from;
      },
      files: () => FileService.connect(phone.request, IOS_ORIGIN),
    });
  },
});
