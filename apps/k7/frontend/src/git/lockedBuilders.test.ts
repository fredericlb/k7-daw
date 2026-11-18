// A locked workspace's builders: the file tree's assembly and chain opens say the lock instead of opening.

import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { usePaneCallbacks } from "../hooks/usePaneCallbacks";

function callbacks(lockedSay: string | null) {
  const i = {
    openEvent: vi.fn(),
    setGraphSlot: vi.fn(),
    cmdRef: { current: null },
    openModuleBuilder: vi.fn(),
    setAssemblyBuilder: vi.fn(),
    setOpenBuilder: vi.fn(),
    setBuilderNode: vi.fn(),
    liveTake: null,
    lockedSay,
    setStatus: vi.fn(),
  };
  let fns!: ReturnType<typeof usePaneCallbacks>;
  const Probe = () => {
    fns = usePaneCallbacks(i);
    return null;
  };
  renderToString(createElement(Probe));
  return { fns, i };
}

describe("the builders the file tree opens", () => {
  it("do not open while locked, and say why", () => {
    const { fns, i } = callbacks("held by ana");
    fns.openAssembly("a.k7x", "lib/a");
    fns.openChain("@me/chain", "n1");
    expect(i.setAssemblyBuilder).not.toHaveBeenCalled();
    expect(i.setOpenBuilder).not.toHaveBeenCalled();
    expect(i.setStatus).toHaveBeenCalledTimes(2);
  });
  it("open when the workspace is writable", () => {
    const { fns, i } = callbacks(null);
    fns.openAssembly("a.k7x", "lib/a");
    fns.openChain("@me/chain");
    expect(i.setAssemblyBuilder).toHaveBeenCalledWith({
      file: "a.k7x",
      folder: "lib/a",
    });
    expect(i.setOpenBuilder).toHaveBeenCalledWith("@me/chain");
  });
});
