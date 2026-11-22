import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { OldKey } from "./OldKey";

const html = (hosts: string[]) =>
  renderToStaticMarkup(
    <OldKey hosts={hosts} fail={() => {}} forgotten={() => {}} />,
  );

describe("the Mac's old key under the shared one", () => {
  it("names the hosts that still take only the old key", () => {
    const out = html(["nas.local:22", "git.example:2222"]);
    expect(out).toContain("still know only this Mac&#x27;s old key");
    expect(out).toContain("nas.local:22, git.example:2222");
    expect(out).toContain("Forget this Mac&#x27;s old key");
  });

  it("says the old key is kept when no host needed it yet, and asks nothing before the button", () => {
    const out = html([]);
    expect(out).toContain("also keeps its old key");
    expect(out).not.toContain("Only the old key is deleted");
  });
});
