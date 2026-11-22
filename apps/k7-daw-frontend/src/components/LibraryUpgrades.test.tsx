// The strip above the file tree: one line for a library played at another version than its pin, whose
// button pins the version played; a held pin keeps the upgrade line it had.

import type { ModuleLibrary } from "@studio/web-protocol";
import type { ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  LibraryUpgrades,
  pinSubstitutions,
  SubstitutionLine,
} from "./LibraryUpgrades";

const lib = (version: ModuleLibrary["version"]): ModuleLibrary => ({
  ns: "k7-core",
  path: "k7-core/modules",
  exists: true,
  nodes: [],
  version,
});

const render = (levels: ModuleLibrary[]) =>
  renderToStaticMarkup(
    <LibraryUpgrades levels={levels} onUpgrade={async () => {}} />,
  );

describe("a pinned version this device lacks", () => {
  const missing = lib({ pinned: 1, requires: 1, used: 8 });

  it("says it once, with the version used and a button that names it", () => {
    const html = render([missing]);
    expect(html).toContain(
      "This workspace pins k7-core 1, which this device doesn&#x27;t have; it opens with k7-core 8. Sounds may differ where a module changed.",
    );
    expect(html).toContain("Pin k7-core 8");
    expect(html).not.toContain("is ready");
    expect(pinSubstitutions([missing])).toEqual([
      { library: "k7-core", pinned: 1, used: 8 },
    ]);
  });

  it("pins the version used when the button is pressed", () => {
    const pinned: unknown[] = [];
    const line = SubstitutionLine({
      s: { library: "k7-core", pinned: 1, used: 8 },
      busy: null,
      onPin: (s) => pinned.push(s),
    });
    const kids = (line.props as { children: ReactElement[] }).children;
    const button = kids[kids.length - 1] as ReactElement<{
      onClick: () => void;
    }>;
    button.props.onClick();
    expect(pinned).toEqual([{ library: "k7-core", pinned: 1, used: 8 }]);
  });
});

describe("a held pin", () => {
  it("shows no substitution, and the upgrade line as before", () => {
    const html = render([lib({ pinned: 2, requires: 1, upgrade: 8 })]);
    expect(html).not.toContain("this device");
    expect(html).toContain("@k7-core plays v2 — v8 is ready");
    expect(pinSubstitutions([lib({ pinned: "draft" })])).toEqual([]);
  });
});
