import { describe, expect, it } from "vitest";
import {
  emptyForm,
  type FormFields,
  formAfter,
  isWebAddress,
  ready,
  request,
  sshHost,
} from "./connectState";

const typed = (f: FormFields, value: string) =>
  formAfter(f, { type: "edit", value });

const ssh = typed(emptyForm, "git@h.org:me/r.git");

describe("the host key's trust", () => {
  const first = formAfter(ssh, {
    type: "unknownHost",
    fingerprint: "SHA256:x",
  });

  it("blocks the call until the person confirms, then sends the fingerprint", () => {
    expect(ready(first)).toBe(false);
    const ok = formAfter(first, { type: "confirm" });
    expect(ready(ok)).toBe(true);
    expect(request(ok).hostKey).toBe("SHA256:x");
  });

  it("asks again when the address changes", () => {
    const ok = formAfter(first, { type: "confirm" });
    expect(typed(ok, "ssh://other/r").trust).toBeNull();
  });

  it("sends no hostKey before a confirmation", () => {
    expect(request(first).hostKey).toBeUndefined();
  });

  it("offers no way on over a changed key, until the old one is forgotten", () => {
    const changed = formAfter(ssh, {
      type: "hostKeyChanged",
      expected: "a",
      got: "b",
    });
    expect(ready(changed)).toBe(false);
    expect(ready(formAfter(changed, { type: "hostForgotten" }))).toBe(true);
  });
});

describe("what a call needs", () => {
  it("needs an address", () => {
    expect(ready(emptyForm)).toBe(false);
    expect(ready(ssh)).toBe(true);
    expect(request(ssh)).toEqual({ url: "git@h.org:me/r.git" });
  });

  it("refuses an https, http or git address and names no field to sign in with", () => {
    for (const url of [
      "https://h.org/r.git",
      "http://h.org/r.git",
      "git://h.org/r.git",
      " HTTPS://h.org/r.git",
    ]) {
      expect(isWebAddress(url)).toBe(true);
      expect(ready(typed(emptyForm, url))).toBe(false);
    }
    expect(isWebAddress("ssh://git@h.org/r.git")).toBe(false);
    expect(Object.keys(emptyForm).sort()).toEqual(["changed", "trust", "url"]);
  });

  it("leaves a file address to the server's own refusal", () => {
    expect(ready(typed(emptyForm, "file:///srv/r.git"))).toBe(true);
  });
});

describe("the host an ssh address names", () => {
  it("reads both spellings", () => {
    expect(sshHost("ssh://git@h.org:2222/me/r.git")).toEqual({
      host: "h.org",
      port: 2222,
    });
    expect(sshHost("git@h.org:me/r.git")).toEqual({ host: "h.org" });
    expect(sshHost("https://h.org/r.git")).toBeNull();
    expect(sshHost("/srv/r.git")).toBeNull();
  });
});
