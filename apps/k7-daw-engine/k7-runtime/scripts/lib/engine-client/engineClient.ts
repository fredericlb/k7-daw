// engineClient.ts — the full engine link for the bench scripts: everything in EngineControl plus sendProject.
// A reader would get wrong that the doc is folded here: a script builds a doc with no groups, and the backend folds the app's.

import type { ProjectDoc } from "@studio/engine-protocol/project";
import { EngineControl } from "./engineControl";

// Re-exported so an importer of `engineClient.ts` gets the control types too, without a second import.
export type {
  AudioDeviceInfo,
  EditPluginInfo,
  EngineState,
  PluginInfo,
  PluginParamInfo,
  PluginScanStatus,
  RecordedTake,
  TransportAction,
} from "./engineControl";
export { EngineControl } from "./engineControl";

export class EngineClient extends EngineControl {
  sendProject(doc: ProjectDoc): void {
    this.send({ type: "project", doc });
  }
}
