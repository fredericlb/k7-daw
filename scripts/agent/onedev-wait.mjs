// onedev-wait.mjs <sha> <branch> [count] — waits on a commit's ci/<branch> and ci/linux/<branch> builds on OneDev, macOS jobs aside.
// It ends as soon as a Windows build fails (exit 1), or when every build has finished (exit 0 if all passed, else 1).
// What a reader gets wrong: the token is read from ~/.claude/onedev (login `fred`), never from an argument (docs/agent-playbook.md).
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const [sha, branch, count = "3"] = process.argv.slice(2);
if (!sha || !branch) {
  console.error("usage: onedev-wait.mjs <sha> <branch> [count]");
  process.exit(2);
}
const server = process.env.ONEDEV_URL ?? "https://dev.frediver.se";
const token = readFileSync(join(homedir(), ".claude", "onedev"), "utf8").trim();
const auth = `Basic ${Buffer.from(`fred:${token}`).toString("base64")}`;
// IGNORE lists build numbers to leave out, such as one the agent itself broke before any code ran.
const ignored = new Set(
  (process.env.IGNORE ?? "").split(/\s+/).filter(Boolean),
);
const query = `"Project" is "frediverse/k7" and ("Branch" is "ci/${branch}" or "Branch" is "ci/linux/${branch}")`;
const url = `${server}/~api/builds?${new URLSearchParams({ query, offset: "0", count: "16" })}`;
const running = new Set(["WAITING", "PENDING", "RUNNING"]);
const seen = new Set();

for (let round = 0; round < 240; round++) {
  await new Promise((r) => setTimeout(r, 45_000));
  let builds;
  try {
    const res = await fetch(url, { headers: { Authorization: auth } });
    builds = (await res.json()).filter(
      (b) =>
        b.commitHash.startsWith(sha) &&
        !b.jobName.includes("macos") &&
        !ignored.has(String(b.number)),
    );
  } catch {
    continue;
  }
  const done = builds.filter((b) => !running.has(b.status));
  for (const b of done) {
    if (seen.has(b.number)) continue;
    seen.add(b.number);
    console.log(
      b.number,
      b.jobName,
      b.status,
      b.refName.replace("refs/heads/", ""),
    );
  }
  if (
    done.some((b) => b.jobName.includes("windows") && b.status !== "SUCCESSFUL")
  )
    process.exit(1);
  if (builds.length >= Number(count) && done.length === builds.length) {
    process.exit(done.every((b) => b.status === "SUCCESSFUL") ? 0 : 1);
  }
}
console.log("timed out waiting for builds");
process.exit(1);
