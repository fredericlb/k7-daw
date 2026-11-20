// shims/fsPromises.ts — node:fs/promises for the browser: the sync calls behind a promise, for the paths that await.

import * as fs from "./fs.ts";

export const readFile = async (p: string, o?: unknown) => fs.readFileSync(p, o);
export const writeFile = async (p: string, d: unknown, o?: unknown) =>
  fs.writeFileSync(p, d, o);
export const mkdir = async (p: string, o?: { recursive?: boolean }) =>
  fs.mkdirSync(p, o);
export const rm = async (
  p: string,
  o?: { recursive?: boolean; force?: boolean },
) => fs.rmSync(p, o);
export const rename = async (a: string, b: string) => fs.renameSync(a, b);
export const stat = async (p: string) => fs.statSync(p);
export const lstat = async (p: string) => fs.lstatSync(p);
export const readdir = async (p: string, o?: { withFileTypes?: boolean }) =>
  fs.readdirSync(p, o);
export const copyFile = async (a: string, b: string, _mode?: number) =>
  fs.copyFileSync(a, b);
export const cp = async (a: string, b: string, o?: { recursive?: boolean }) =>
  fs.cpSync(a, b, o);
export const unlink = async (p: string) => fs.unlinkSync(p);
export const access = async (p: string) => {
  fs.statSync(p);
};
export const mkdtemp = async (p: string) => fs.mkdtempSync(p);
export const realpath = async (p: string) => fs.realpathSync(p);
export const utimes = async (p: string, a: number | Date, m: number | Date) =>
  fs.utimesSync(p, a, m);

export default {
  readFile,
  writeFile,
  mkdir,
  rm,
  rename,
  stat,
  lstat,
  readdir,
  copyFile,
  cp,
  unlink,
  access,
  mkdtemp,
  realpath,
  utimes,
};
