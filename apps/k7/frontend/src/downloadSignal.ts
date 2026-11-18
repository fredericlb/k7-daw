// downloadSignal.ts — wait for the "download started" cookie the tracks route sets on its first byte (`k7dl-<token>`).
// What a reader gets wrong: `ok` means the answer began, not that the file is saved; `err.<sentence>` is a refusal.

/** The cookie's name starts with this, then the request's token. */
const PREFIX = "k7dl-";

export type Started =
  | { kind: "started" }
  | { kind: "refused"; message: string }
  | { kind: "silent" };

/** A request token: letters and digits only, as the route accepts. */
export function newToken(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

function said(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function cookieOf(jar: string, name: string): string | null {
  for (const part of jar.split("; ")) {
    if (part.startsWith(`${name}=`)) return part.slice(name.length + 1);
  }
  return null;
}

/** Poll for the cookie of `token`; `silent` after `timeoutMs` (a route that did not answer natively sets none). */
export function awaitStart(
  token: string,
  { pollMs = 250, timeoutMs = 15 * 60_000, jar = () => document.cookie } = {},
): Promise<Started> {
  const name = `${PREFIX}${token}`;
  const until = Date.now() + timeoutMs;
  return new Promise((done) => {
    const look = () => {
      const value = cookieOf(jar(), name);
      if (value !== null) {
        done(
          value.startsWith("err.")
            ? { kind: "refused", message: said(value.slice(4)) }
            : { kind: "started" },
        );
      } else if (Date.now() >= until) done({ kind: "silent" });
      else setTimeout(look, pollMs);
    };
    look();
  });
}
