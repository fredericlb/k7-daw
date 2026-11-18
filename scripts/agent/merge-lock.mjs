// merge-lock.mjs — one agent merges into win/main at a time: `take <branch>` waits for the lock, `give <branch>` frees it.
// The lock is a file beside the agents' Windows scripts; one older than 45 minutes is taken as abandoned (docs/agents.md).
import { closeSync, openSync, readFileSync, rmSync, writeSync } from "node:fs";

const LOCK = "/mnt/c/Users/flb/AppData/Local/Temp/k7-verify/merge.lock";
const STALE_MS = 45 * 60_000;
const [verb, branch] = process.argv.slice(2);

const holder = () => {
  try {
    const [who, at] = readFileSync(LOCK, "utf8").trim().split(" ");
    return { who, at: Number(at) };
  } catch {
    return null;
  }
};

const tryTake = () => {
  try {
    const fd = openSync(LOCK, "wx");
    writeSync(fd, `${branch} ${Date.now()}\n`);
    closeSync(fd);
    return true;
  } catch {
    return false;
  }
};

if (!branch || !["take", "give", "who"].includes(verb)) {
  console.error(
    "usage: node scripts/agent/merge-lock.mjs take|give|who <branch>",
  );
  process.exit(2);
}
if (verb === "who") {
  const h = holder();
  console.log(h ? `${h.who} since ${new Date(h.at).toISOString()}` : "free");
} else if (verb === "give") {
  const h = holder();
  if (h && h.who !== branch) {
    console.error(`merge-lock: held by ${h.who}, not ${branch}; left as it is`);
    process.exit(1);
  }
  rmSync(LOCK, { force: true });
  console.log(`merge-lock: ${branch} gave it back`);
} else {
  for (;;) {
    if (tryTake()) break;
    const h = holder();
    if (h && Date.now() - h.at > STALE_MS) {
      console.log(`merge-lock: ${h.who} held it past 45 minutes; taking it`);
      rmSync(LOCK, { force: true });
      continue;
    }
    console.log(`merge-lock: ${h?.who ?? "someone"} is merging; waiting`);
    await new Promise((r) => setTimeout(r, 30_000));
  }
  console.log(`merge-lock: ${branch} holds it`);
}
