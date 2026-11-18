// server.ts — k7's TypeScript backend behind the Rust front: the session it reads, the account's preferences,
// the bearer and the seam's file half (./media/mount.ts), and the built SPA — docker/ ships no nginx. A
// former host's page goes to k7's origin, to the home. The engine core, the
// shares and the file hub are the front's (apps/k7/backend-rs/docs/).
// Runs under tsx to import the brain's TS.

import { createServer } from "node:http";
import type { Socket } from "node:net";
import { homedir } from "node:os";
import { join } from "node:path";
import { INTERNAL_PATH, internalGate } from "@studio/engine-registry/seam";
import { seamConfigFromEnv } from "@studio/engine-registry/seam-client";
import { loadServiceEnv } from "@studio/env";
import { precompressedStatic } from "@studio/http/precompressed";
import { behindProxy } from "@studio/http/proxied";
import { setupAuth } from "@studio/oidc";
import { prefsStore } from "@studio/user-prefs";
import { preferencesRouter } from "@studio/user-prefs/router";
import { isLegacyHost, serviceUrl } from "@studio/web-protocol/services";
import express from "express";
import { z } from "zod";
import { authRefusals } from "./authGuard.ts";
import { sessionRevocations } from "./authState.ts";
import { jsonBodies } from "./bodyLimits.ts";
import { originPolicyFromEnv, studioCors } from "./cors.ts";
import { withDevAdmin } from "./devAdmin.ts";
import { devFrontend } from "./devFrontend.ts";
import { trustFront } from "./front.ts";
import {
  jsonErrorHandler,
  securityHeaders,
  shellFrameHeaders,
} from "./hardening.ts";
import { LOCAL_USER as LOCAL_SESSION_USER } from "./localUser.ts";
import { loopbackOnly } from "./loopback.ts";
import { mediaTokens, mountMedia } from "./media/mount.ts";
import { siteHostsFromEnv, sitePage } from "./sitePage.ts";
import { guardUpgrades } from "./upgradeGuard.ts";
import { BUILD, VERSION } from "./version.ts";

// A stray rejection is logged and survived; an uncaught exception leaves, and the container restarts it.
process.on("unhandledRejection", (reason) =>
  console.error("unhandled rejection:", reason),
);
process.on("uncaughtException", (err) => {
  console.error("uncaught exception:", err);
  process.exit(1);
});

// Dev config layered INTO process.env, never read into a value — ../docs/config.md.
Object.assign(process.env, loadServiceEnv("k7-backend"));

// No dev-user stub and no guessable session secret on a server — ../docs/hardening.md.
const refusals = authRefusals(process.env);
if (refusals.length > 0) {
  for (const r of refusals) console.error(`FATAL: ${r}`);
  console.error(
    "FATAL: refusing to start; set STUDIO_DEV_AUTH=1 only for a local dev session",
  );
  process.exit(1);
}

const PORT = Number(process.env.PORT || 8789);
// Unset, every interface; the desktop and the phone set 127.0.0.1 (../docs/standalone.md).
const LISTEN_HOST = process.env.STUDIO_LISTEN_HOST || undefined;
const LOCAL_USER = process.env.STUDIO_LOCAL_USER === "1";
const app = express();
// Express's own banner tells a client nothing it needs.
app.disable("x-powered-by");
app.use(securityHeaders);
// The internal seam, ahead of the loopback lock and CORS: its own gate is loopback AND the secret (packages/engine-registry/docs/internalSeam.md).
const seamConfig = seamConfigFromEnv(PORT);
const internal = express.Router();
app.use(INTERNAL_PATH, internalGate(seamConfig.secret), internal);

// A backend that signs anyone in as the local user answers its own machine, by address and by name.
if (LOCAL_USER) app.use(loopbackOnly);
// Listed origins only, and no cross-site write, before anything else runs — ../docs/cors.md.
app.use(studioCors(originPolicyFromEnv()));
// Liveness: no session, no disk, only that the loop answers — and which release answers.
app.get("/api/health", (_req, res) => {
  res.json({ ok: true, version: VERSION, build: BUILD });
});
// Small for everyone, since it runs before any guard (./bodyLimits.ts).
app.use(jsonBodies());
// The root domain and www.: one page, before the session and everything else (./sitePage.ts).
app.use(sitePage(siteHostsFromEnv()));

// Empty means k7's own default, not the package's: without `groups` every session is no admin.
const scopes = (process.env.OIDC_SCOPES || "openid profile email groups")
  .split(/[\s,]+/)
  .filter((s) => s !== "");

// This backend OWNS /auth/* for the whole origin, being the service with no prefix (../../AGENTS.md).
const auth = await setupAuth(app, {
  cookieName: "studio.sid",
  // A logout everywhere outlives a restart (../docs/sessions.md).
  revocations: sessionRevocations(),
  // The env.yaml section, which is the PACKAGE name here as everywhere else.
  service: "k7-backend",
  // Dropping `groups` would 403 every admin surface — see ../../AGENTS.md, § The backend in detail.
  scopes,
  // The sign-in is the Rust front's; `/auth/me` stays, the question its warm-up asks here (apps/k7/backend-rs/docs/routes.md).
  mountRoutes: ["me"],
});
// A machine has ONE account, its local user: who it is at its server is the link's (apps/k7/backend-rs/docs/files.md).
if (LOCAL_USER)
  app.use((req, _res, next) => {
    const user = auth.sessionUser(req);
    if (user && !user.local) auth.startSession(req, LOCAL_SESSION_USER);
    next();
  });
// A local dev session's stub user is an admin, nobody else's is (./devAdmin.ts).
const requireAuth = withDevAdmin(auth.requireAuth);
// Under the home directory so a dev run needs no volume (packages/user-prefs/docs/store.md).
const PREFS_DIR =
  process.env.STUDIO_PREFS_DIR || join(homedir(), ".studio", "prefs", "k7");
// Before the prefs below, whose `/api` guard is session-only: a library bearer reaches them through this one.
mountMedia(app, { requireAuth, tokens: mediaTokens(), internal });

/** One person's layer over a command, as the WIRE shape — the meaning is the frontend's `commandPrefs.ts`. */
const commandPrefSchema = z.object({
  label: z.union([z.string(), z.record(z.string(), z.string())]).optional(),
  icon: z.string().optional(),
  order: z.number().optional(),
  key: z.string().optional(),
  // Which right-clicks may offer it: a list of entities, or `false` for none — see the frontend's commandPrefs.ts.
  menu: z.union([z.boolean(), z.array(z.string())]).optional(),
});

/** What this service remembers about a person — the GATE, not a description. */
const prefsSchema = z.object({
  engineId: z.string().optional(),
  commands: z.record(z.string(), commandPrefSchema).optional(),
  // Read by nothing: declared so an account's stored document still passes this gate on its next save.
  playbackTarget: z.string().optional(),
  // Kept so a wholesale save from an older page still passes; nothing reads them.
  favorites: z.array(z.string()).optional(),
});
app.use(
  "/api",
  requireAuth,
  preferencesRouter(
    prefsStore<z.infer<typeof prefsSchema>>(PREFS_DIR),
    prefsSchema,
  ),
);

// The old hosts, kept for saved links and installs: a page moves to k7's origin, to the home; the API keeps answering.
const ALIASES: Record<string, string | null> = {
  "play.": null,
  "k7.": null,
};
app.use((req, res, next) => {
  if (
    !isLegacyHost(req.hostname) ||
    req.path.startsWith("/api") ||
    req.path.startsWith("/auth")
  )
    return next();
  const alias = Object.keys(ALIASES).find((h) => req.hostname.startsWith(h));
  const root = serviceUrl(`${req.protocol}://${req.hostname}`, "k7");
  const rest = req.originalUrl === "/" ? "" : req.originalUrl;
  const prefix = alias ? ALIASES[alias] : "";
  res.redirect(301, prefix === null ? `${root}/` : `${root}${prefix}${rest}`);
});

// The built SPA: a real file wins outright, then the shell for everything but /api and /auth, which 404 on their own.
const FRONTEND_DIST = join(import.meta.dirname, "../../frontend/dist");
// The shell is asked again on every load and a hashed asset is kept for good — ../../../../docs/versioning.md.
const SHELL_CACHE = "no-cache";
// …or, in the dev tray, a running Vite in its place, for HMR on this origin (./devFrontend.ts).
const DEV_FRONTEND = process.env.K7_DEV_FRONTEND || "";
if (DEV_FRONTEND) app.use(devFrontend(DEV_FRONTEND));
else {
  app.use(
    precompressedStatic(FRONTEND_DIST, {
      setHeaders: (res, path) => {
        res.setHeader(
          "Cache-Control",
          path.startsWith(join(FRONTEND_DIST, "assets"))
            ? "public, max-age=31536000, immutable"
            : SHELL_CACHE,
        );
        if (path.endsWith(".html")) shellFrameHeaders(res);
      },
    }),
  );
  // Not for /assets/: a hashed chunk that is gone must 404, or the page parses index.html as a script.
  app.get(/^(?!\/api|\/auth|\/assets\/)/, (_req, res) => {
    res.setHeader("Cache-Control", SHELL_CACHE);
    shellFrameHeaders(res);
    res.sendFile(join(FRONTEND_DIST, "index.html"));
  });
}

// Mounted LAST: a JSON answer for every error, and never a 5xx's message or stack.
app.use(jsonErrorHandler);

const onListening = () => {
  const url = `http://${LISTEN_HOST ?? "localhost"}:${PORT}`;
  console.log(`🎛  k7 backend on ${url}  (preferences + the front's seam)`);
  // A host that asked is told on stdout that requests are answered now, rather than polling (engine/k7-runtime/docs/android.md).
  if (process.env.STUDIO_READY_LINE) console.log(process.env.STUDIO_READY_LINE);
};
const server = behindProxy(createServer(app));
// Before any listener: an upgrade nobody serves is refused, and a reset socket never ends the process (../docs/upgrades.md).
guardUpgrades(server);
// Behind the Rust front every socket is loopback: the peer it vouches for is the one the checks read (./front.ts).
if (process.env.K7_FRONT_SECRET)
  trustFront(server, process.env.K7_FRONT_SECRET);
const listen = (): void => {
  if (LISTEN_HOST) server.listen(PORT, LISTEN_HOST, onListening);
  else server.listen(PORT, onListening);
};
listen();
// Node's default cuts a request at 5 minutes, and a 512 MB upload over a slow link takes longer.
server.requestTimeout = 30 * 60_000;

// Every socket, upgraded ones included: `server.close` waits for each, and an upgraded one never ends by itself.
const sockets = new Set<Socket>();
server.on("connection", (s: Socket) => {
  sockets.add(s);
  s.once("close", () => sockets.delete(s));
});

// A stop lets the requests in flight finish, within the 10 s Docker allows before its SIGKILL.
let stopping = false;
const stop = (): void => {
  if (stopping) return;
  stopping = true;
  console.log("stopping: the requests in flight");
  setTimeout(() => process.exit(0), 4_000).unref();
  server.close(() => process.exit(0));
  server.closeIdleConnections();
  // What is still open a second later is a socket nobody will end: cut, so the close completes inside the budget.
  setTimeout(() => {
    for (const s of sockets) s.destroy();
  }, 1_000).unref();
};
for (const signal of ["SIGTERM", "SIGINT"] as const) process.on(signal, stop);
// Under the tray, stdin closing is the stop Windows has no signal for, and how a dead tray takes us down elsewhere.
const STDIN_STOP = ["K7_STDIN_STOP", "ELECTRON_SHELL_STDIN_STOP"];
if (STDIN_STOP.some((name) => process.env[name] === "1")) {
  for (const name of STDIN_STOP) delete process.env[name];
  process.stdin.on("end", stop);
  process.stdin.resume();
}
