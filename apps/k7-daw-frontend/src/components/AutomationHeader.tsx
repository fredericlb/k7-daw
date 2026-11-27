// AutomationHeader.tsx — an automation lane's header in the timeline's gutter: its param's name. Removing the lane is
// its right-click menu's "Remove automation" (the curve-point menu), which the row opens (docs/timeline.md).

export function AutomationHeader({
  module,
  param,
  curve,
  width,
  className,
  gutter,
}: {
  module: string;
  param: string;
  curve: string;
  width: number;
  className: string;
  /** The gutter's own press, click and double-click: the row is selected and dragged as any other. */
  gutter: {
    onPointerDown: (e: React.PointerEvent) => void;
    onClick: (e: React.MouseEvent) => void;
    onDoubleClick: () => void;
  };
}) {
  return (
    <div
      style={{ width }}
      {...gutter}
      className={className}
      title={`${module} · ${param} ← ${curve}`}
    >
      <span className="truncate">{param}</span>
    </div>
  );
}
