// shims/childProcess.ts — node:child_process for the browser: every call says the work belongs to the server.
// What a reader gets wrong: nothing here runs; a verb that spawns is a host member's (3f), and this keeps a stray import loud.

const no = (name: string) => (): never => {
  throw new Error(
    `${name}: a browser starts no process; this verb needs the server`,
  );
};

export const spawn = no("spawn");
export const spawnSync = no("spawnSync");
export const execFile = no("execFile");
export const execFileSync = no("execFileSync");
export const exec = no("exec");
export const execSync = no("execSync");
export default { spawn, spawnSync, execFile, execFileSync, exec, execSync };
