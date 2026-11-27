// widgetKinds.ts — what the grid editor draws for each widget KIND: its palette entry and its icon.
// Whether a kind binds or carries a range, and which fields survive a change of type, are Rust's (`@studio/k7-wasm/panel`).

import {
  Activity,
  AudioWaveform,
  Circle,
  FileSearch,
  Group,
  Hash,
  List,
  type LucideIcon,
  Monitor,
  RectangleHorizontal,
  SlidersHorizontal,
  SlidersVertical,
  ToggleLeft,
  Type,
} from "@studio/ui/icons";
import type { WidgetType } from "./moduleUi";

export const TYPES: { type: WidgetType; icon: LucideIcon; label: string }[] = [
  { type: "knob", icon: Circle, label: "Knob" },
  { type: "slider", icon: SlidersHorizontal, label: "Slider" },
  { type: "fader", icon: SlidersVertical, label: "Fader" },
  { type: "toggle", icon: ToggleLeft, label: "Toggle" },
  { type: "dropdown", icon: List, label: "Dropdown" },
  { type: "filepicker", icon: FileSearch, label: "File" },
  { type: "button", icon: RectangleHorizontal, label: "Button" },
  { type: "number", icon: Hash, label: "Number" },
  { type: "meter", icon: Activity, label: "Meter" },
  { type: "waveform", icon: AudioWaveform, label: "Waveform" },
  { type: "lcdscreen", icon: Monitor, label: "LCD screen" },
  { type: "label", icon: Type, label: "Label" },
  { type: "group", icon: Group, label: "Group" },
];

export const ICON: Record<WidgetType, LucideIcon> = Object.fromEntries(
  TYPES.map((t) => [t.type, t.icon]),
) as Record<WidgetType, LucideIcon>;

export { bindable, GROUP_COLOR, ranged, retype } from "@studio/k7-wasm/panel";
