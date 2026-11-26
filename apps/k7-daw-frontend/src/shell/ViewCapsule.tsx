// ViewCapsule.tsx — the current pane's view options (lanes, automation, zoom; the graph's fit, layout, stack) in a small
// capsule among the floating controls at the bottom right (ProjectHeader.tsx), icons only; none on the other panes, and a phone's menu holds them.
import { CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import {
  Columns3,
  Image,
  Maximize,
  Network,
  Rows3,
  Spline,
  ZoomIn,
  ZoomOut,
} from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";

export interface BarLaneViews {
  expanded: boolean;
  automation: boolean;
  /** Only on a project with a picture lane: the picture card's toggle. */
  picture: boolean | null;
  onLanes: () => void;
  onAutomation: () => void;
  onPicture: () => void;
  /** The timeline's zoom, the keys z, Shift+Z and 0 as buttons. */
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomFit: () => void;
}

export interface BarGraphViews {
  onFit: () => void;
  /** Absent where the host offers no auto layout. */
  onOrganize?: () => void;
  onStack?: () => void;
  stacked: boolean;
}

// A small capsule: 36 px tall, 28 px buttons, 16 px icons.
const CAPSULE =
  "float pointer-events-auto flex h-9 items-center gap-0.5 rounded-full p-1";
const BTN =
  "h-7 min-w-7 px-1.5 desk:h-7 desk:min-w-7 desk:rounded-full rounded-full [&_svg]:size-4 desk:[&_svg]:size-4";

export function ViewCapsule({
  lanes,
  graph,
}: {
  /** The arrangement's options; null on any other pane. */
  lanes: BarLaneViews | null;
  /** The Graph pane's options; null on any other pane (docs/shell.md, "The capsule follows the pane"). */
  graph: BarGraphViews | null;
}) {
  const laneViews = lanes ? (
    <div role="group" aria-label={m["shell.laneViews"]()} className={CAPSULE}>
      <CapsuleButton
        className={BTN}
        pressed={lanes.expanded}
        aria-label={m["shell.lane.lanes"]()}
        title={m["shell.lane.lanes"]()}
        onClick={lanes.onLanes}
      >
        <Rows3 />
      </CapsuleButton>
      <CapsuleButton
        className={BTN}
        pressed={lanes.automation}
        aria-label={m["shell.lane.automation"]()}
        title={m["shell.lane.automation"]()}
        onClick={lanes.onAutomation}
      >
        <Spline />
      </CapsuleButton>
      <CapsuleDivider />
      <CapsuleButton
        className={BTN}
        aria-label={m["shell.lane.zoomOut"]()}
        title={m["shell.lane.zoomOut"]()}
        onClick={lanes.onZoomOut}
      >
        <ZoomOut />
      </CapsuleButton>
      <CapsuleButton
        className={BTN}
        aria-label={m["shell.lane.zoomIn"]()}
        title={m["shell.lane.zoomIn"]()}
        onClick={lanes.onZoomIn}
      >
        <ZoomIn />
      </CapsuleButton>
      <CapsuleButton
        className={BTN}
        aria-label={m["shell.lane.zoomFit"]()}
        title={m["shell.lane.zoomFit"]()}
        onClick={lanes.onZoomFit}
      >
        <Maximize />
      </CapsuleButton>
      {lanes.picture !== null && (
        <CapsuleButton
          className={BTN}
          pressed={lanes.picture}
          aria-label={m["shell.lane.pictureName"]()}
          title={m["shell.lane.pictureName"]()}
          onClick={lanes.onPicture}
        >
          <Image />
        </CapsuleButton>
      )}
    </div>
  ) : null;

  const graphViews = graph ? (
    <div role="group" aria-label={m["shell.graphViews"]()} className={CAPSULE}>
      <CapsuleButton
        className={BTN}
        aria-label={m["graph.zoomFitName"]()}
        title={m["graph.zoomFitName"]()}
        onClick={graph.onFit}
      >
        <Maximize />
      </CapsuleButton>
      {graph.onOrganize && (
        <CapsuleButton
          className={BTN}
          aria-label={m["graph.autoLayoutName"]()}
          title={m["graph.autoLayoutName"]()}
          onClick={graph.onOrganize}
        >
          <Network />
        </CapsuleButton>
      )}
      {graph.onStack && (
        <CapsuleButton
          className={BTN}
          pressed={graph.stacked}
          aria-label={m["graph.stackName"]()}
          title={m["graph.stackName"]()}
          onClick={graph.onStack}
        >
          <Columns3 />
        </CapsuleButton>
      )}
    </div>
  ) : null;
  return laneViews ?? graphViews;
}
