// shims/os.ts — node:os for the browser: the two folders verbs name, inside the vfs, and the platform the verbs test.

export const homedir = (): string => "/home";
export const tmpdir = (): string => "/tmp";
export const platform = (): string => "linux";
export const EOL = "\n";
export default { homedir, tmpdir, platform, EOL };
