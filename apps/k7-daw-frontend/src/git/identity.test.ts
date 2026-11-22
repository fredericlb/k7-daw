import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("./gitApi", () => ({
  git: {
    setIdentity: vi.fn(async () => ({ ok: true })),
    identity: vi.fn(async () => ({
      name: "Ada",
      email: "a@x.org",
      emailSource: "github",
    })),
  },
}));
vi.mock("../settings/accountFiles", () => ({ findAccountFiles: vi.fn() }));
vi.mock("../paraglide/messages.js", () => ({
  m: { "home.localUser": () => "Local" },
}));

import { git } from "./gitApi";
import { headingOf, saveIdentity, setKnownIdentity } from "./identity";

const t = { base: "/f", headers: () => ({}) };

afterEach(() => setKnownIdentity(null));

describe("the identity", () => {
  it("heads the menu with the name, else the email, else Local", () => {
    expect(headingOf({ name: " Ada ", email: "a@x.org" })).toBe("Ada");
    expect(headingOf({ name: "", email: "a@x.org" })).toBe("a@x.org");
    expect(headingOf({ name: " ", email: "" })).toBe("Local");
    expect(headingOf(null)).toBe("Local");
  });

  it("saves trimmed through the route and tells the readers", async () => {
    const saved = await saveIdentity(t, { name: " Ada ", email: "a@x.org " });
    expect(saved).toEqual({ name: "Ada", email: "a@x.org" });
    expect(git.setIdentity).toHaveBeenCalledWith(t, saved);
    expect(headingOf(saved)).toBe("Ada");
  });
});
