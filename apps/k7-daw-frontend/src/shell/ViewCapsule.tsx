// ViewCapsule.tsx — the current pane's view options (lanes, automation, fit; the graph's fit, layout, stack) in a small
// capsule among the floating controls at the bottom right (ProjectHeader.tsx), icons only; none on the other panes; a phone's rail holds them.
import { CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import {
  Columns3,
  Image,
  Maximize,
  Network,
  Rows3,
  Spline,
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
  /** The timeline's fit, the key 0 as a button; z and Shift+Z zoom from the keyboard. */
  onZoomFit: () => void;
}

export interface BarGraphViews {
  onFit: () => void;
  /** Absent where the host offers no auto layout. */
  onOrganize?: () => void;
  onStack?: () => void;
  stacked: boolean;
}

// As tall as the floating row (40 px), 32 px buttons, 16 px icons.
const CAPSULE =
  "float pointer-events-auto flex h-(--k7-float-h) items-center gap-0.5 rounded-full p-1";

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
        size="float"
        pressed={lanes.expanded}
        aria-label={m["shell.lane.lanes"]()}
        title={m["shell.lane.lanes"]()}
        onClick={lanes.onLanes}
      >
        <Rows3 />
      </CapsuleButton>
      <CapsuleButton
        size="float"
        pressed={lanes.automation}
        aria-label={m["shell.lane.automation"]()}
        title={m["shell.lane.automation"]()}
        onClick={lanes.onAutomation}
      >
        <Spline />
      </CapsuleButton>
      <CapsuleDivider />
      <CapsuleButton
        size="float"
        aria-label={m["shell.lane.zoomFit"]()}
        title={m["shell.lane.zoomFit"]()}
        onClick={lanes.onZoomFit}
      >
        <Maximize />
      </CapsuleButton>
      {lanes.picture !== null && (
        <CapsuleButton
          size="float"
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
        size="float"
        aria-label={m["graph.zoomFitName"]()}
        title={m["graph.zoomFitName"]()}
        onClick={graph.onFit}
      >
        <Maximize />
      </CapsuleButton>
      {graph.onOrganize && (
        <CapsuleButton
          size="float"
          aria-label={m["graph.autoLayoutName"]()}
          title={m["graph.autoLayoutName"]()}
          onClick={graph.onOrganize}
        >
          <Network />
        </CapsuleButton>
      )}
      {graph.onStack && (
        <CapsuleButton
          size="float"
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
