// graph.ts — the Chains routing graph's one import site: model -> nodes/edges -> laid-out geometry. The build, the arrangements
// and the widget maths are Rust's (`@studio/k7-wasm`); the drawing geometry and the layout (dagre) are here. Renderer-agnostic.
// Hosts import "./graph"; the split is internal.

export {
  isMixKnob,
  pairValue,
  subgraphForTrack,
  trackInputs,
  trackModules,
  widgetDenorm,
  widgetNorm,
  widgetText,
  widgetTyped,
} from "@studio/k7-wasm/chains";
export {
  INPUT_VOLUME_PARAM,
  MIX_OUT,
  MIX_PARAM,
  OUTPUT_VOLUME_PARAM,
  RENDER_OUT,
} from "@studio/k7-wasm/consts";
export {
  RACK_ADD_BIND,
  RACK_BIND,
  rackAddBind,
  rackAddBindAt,
  rackBindSymbol,
} from "@studio/k7-wasm/rack-binds";
export { projectBindField } from "./binds";
export { buildGraph, withOpenTabs } from "./buildGraph";
export {
  assemblyId,
  CELL,
  CLIP_LIST_MAX,
  COL_GAP,
  cablePath,
  colsWidth,
  columnW,
  curveOutPoint,
  EVENT_IN_AT,
  eventOutBottom,
  eventPoint,
  FOOTER_H,
  FRAME_HEAD,
  FRAME_PAD,
  HEADER_MAX_PX,
  HEADER_MIN_PX,
  headerFontPx,
  jackPoint,
  MAIN_OUT,
  menuH,
  moduleDisplayName,
  NODE_H,
  NODE_MIN_W,
  NODE_W,
  nodeSize,
  PORT_PLATE,
  ROW_GAP,
  rowsHeight,
  stripWidgetRect,
  TAB_COL_MAX_ROWS,
  TAB_COL_W,
  TAB_H,
  TAB_MENU_H,
  UI_CONTENT_TOP,
  UI_EDGE,
  UI_HEADER,
  UI_PAD,
  UI_TOP_GAP,
  VIDEO_OUT,
  widgetRect,
} from "./geometry";
export {
  layout,
  masterRunAside,
  pushAsideFrames,
  pushBelowInputs,
} from "./graphLayout";
export type {
  EdgeAutomation,
  EdgeInsert,
  GraphEdge,
  GraphNode,
  Jack,
  Kind,
  LaidOutGraph,
  Pt,
  RawEdge,
  RawNode,
  TrackControlValues,
} from "./graphNode";
export { alignModuleRank, moduleRow, spaceModulesEvenly } from "./moduleRow";
