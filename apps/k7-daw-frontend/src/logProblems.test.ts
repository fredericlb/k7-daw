import { describe, expect, it } from "vitest";
import type { LogLine } from "./components/LogsPanel";
import { isCommandLine, isProblemLine } from "./logProblems";

const line = (kind: LogLine["kind"], text: string): LogLine => ({
  t: 0,
  kind,
  text,
});

describe("logProblems", () => {
  it("counts failures and engine lines that say they failed", () => {
    expect(isProblemLine(line("error", "boom"))).toBe(true);
    expect(isProblemLine(line("engine", "[engine] audio device failed"))).toBe(
      true,
    );
    expect(isProblemLine(line("engine", "[engine] listening"))).toBe(false);
  });
  it("files a refusal under Commands, not Problems", () => {
    expect(isProblemLine(line("refused", "no such clip"))).toBe(false);
    expect(isCommandLine(line("refused", "no such clip"))).toBe(true);
    expect(isCommandLine(line("cmd", "› play"))).toBe(true);
  });
});
