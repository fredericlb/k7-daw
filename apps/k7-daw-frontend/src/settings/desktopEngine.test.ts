// desktopEngine.test.ts — the primary and fallback audio interface as the Devices tab draws and sends them.

import { describe, expect, it } from "vitest";
import {
  audioChoice,
  idleAsioDriver,
  instrumentAction,
  instrumentRows,
  priorityArg,
  sizeOf,
} from "./desktopEngine";

const snapshot = {
  status: "running" as const,
  userStopped: false,
  engineId: "e",
  enginePort: 8766,
  lastHeartbeatOk: true,
  audioDevices: [
    {
      name: "Built-in",
      deviceType: "CoreAudio",
      current: true,
      currentInput: true,
      inputs: 0,
      outputs: 0,
    },
    {
      name: "Scarlett",
      deviceType: "CoreAudio",
      current: false,
      currentInput: false,
      inputs: 0,
      outputs: 0,
    },
  ],
  audioPriority: ["MOTU", "Built-in"],
};

describe("the audio interface choice", () => {
  it("reads the primary and the fallback off the ranking, an unplugged one kept as an option", () => {
    expect(audioChoice(snapshot)).toEqual({
      primary: "MOTU",
      fallback: "Built-in",
      options: [
        { name: "Built-in", present: true },
        { name: "Scarlett", present: true },
        { name: "MOTU", present: false },
      ],
    });
  });

  it("starts from the device in use when nothing is ranked", () => {
    const c = audioChoice({ ...snapshot, audioPriority: [] });
    expect([c.primary, c.fallback]).toEqual(["Built-in", null]);
  });

  it("joins the primary and its fallback, and drops a fallback equal to it or none", () => {
    expect(priorityArg("MOTU", "Built-in")).toBe("MOTU\u001fBuilt-in");
    expect(priorityArg("MOTU", null)).toBe("MOTU");
    expect(priorityArg("MOTU", "MOTU")).toBe("MOTU");
  });

  it("names an ASIO driver the engine is not on, and none once it is", () => {
    const windows = (asioCurrent: boolean) => ({
      ...snapshot,
      audioDevices: [
        {
          name: "Focusrite USB ASIO",
          deviceType: "ASIO",
          current: asioCurrent,
          currentInput: asioCurrent,
          inputs: 0,
          outputs: 0,
        },
        {
          name: "Speakers (Focusrite USB Audio)",
          deviceType: "Windows Audio",
          current: !asioCurrent,
          currentInput: !asioCurrent,
          inputs: 0,
          outputs: 0,
        },
      ],
    });
    expect(idleAsioDriver(windows(false))).toBe("Focusrite USB ASIO");
    expect(idleAsioDriver(windows(true))).toBeNull();
    expect(idleAsioDriver(snapshot)).toBeNull();
  });
});

describe("the instruments", () => {
  const s = {
    ...snapshot,
    instruments: [
      {
        name: "piano",
        label: "Piano",
        bytes: 748e6,
        wanted: true,
        installed: false,
      },
      {
        name: "drums",
        label: "Drums",
        bytes: 1.3e9,
        wanted: true,
        installed: true,
      },
      {
        name: "clav",
        label: "Clavinet",
        bytes: 8e6,
        wanted: false,
        installed: false,
      },
    ],
    downloads: [
      {
        name: "piano",
        label: "Piano",
        received: 1e6,
        total: 748e6,
        phase: "downloading" as const,
      },
    ],
  };

  it("joins each instrument with its download, and offers the one action that fits", () => {
    const rows = instrumentRows(s);
    expect(
      rows.map((r) => [r.name, r.phase, instrumentAction(r)?.label ?? null]),
    ).toEqual([
      ["piano", "downloading", null],
      ["drums", null, "remove"],
      ["clav", null, "download"],
    ]);
    expect(instrumentAction({ ...rows[0], phase: "queued" })?.cmd).toBe(
      "instruments.remove",
    );
    expect(instrumentAction({ ...rows[0], phase: "failed" })?.label).toBe(
      "retry",
    );
  });

  it("says a size the way a download costs it", () => {
    expect(sizeOf(748e6)).toBe("748 MB");
    expect(sizeOf(1.3e9)).toBe("1.3 GB");
    expect(sizeOf(10)).toBe("1 MB");
  });
});
