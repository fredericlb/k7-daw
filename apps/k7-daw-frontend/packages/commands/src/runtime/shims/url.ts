// shims/url.ts — node:url's file URL helpers for the browser; a bundled module's `import.meta.url` names no real file.

export function fileURLToPath(u: string | { href: string }): string {
  const s = typeof u === "string" ? u : u.href;
  return decodeURIComponent(s.replace(/^file:\/\/(\/(?=[A-Za-z]:))?/, ""));
}

export const pathToFileURL = (p: string): { href: string } => ({
  href: `file://${p.startsWith("/") ? "" : "/"}${encodeURI(p.replaceAll("\\", "/"))}`,
});
export default { fileURLToPath, pathToFileURL };
