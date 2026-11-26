// ToolMenu.tsx — the timeline's tool picker, in its corner left of "+ lane", and the tool names and glyphs a phone's menu shares.
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import {
  Check,
  ChevronDown,
  Eraser,
  type LucideIcon,
  MousePointer2,
  Scissors,
  VolumeX,
} from "@studio/ui/icons";
import { ModeIcon } from "../modeIcons";
import { m } from "../paraglide/messages.js";
import { TOOLS, type Tool } from "../timelineTools";

export const TOOL_NAME: Record<Tool, () => string> = {
  pointer: () => m["shell.tool.pointer"](),
  cut: () => m["shell.tool.cut"](),
  erase: () => m["shell.tool.erase"](),
  mute: () => m["shell.tool.mute"](),
  crossfade: () => m["shell.tool.crossfade"](),
};

const TOOL_ICON: Record<Tool, LucideIcon | null> = {
  pointer: MousePointer2,
  cut: Scissors,
  erase: Eraser,
  mute: VolumeX,
  crossfade: null,
};

export function ToolGlyph({ tool }: { tool: Tool }) {
  const Icon = TOOL_ICON[tool];
  return Icon ? <Icon /> : <ModeIcon mode="crossfade" className="size-5" />;
}

// The corner's pill: the current tool's glyph and a chevron, drawn like the "+ lane" button beside it.
export function ToolMenu({
  tool,
  onTool,
  className,
}: {
  tool: Tool;
  onTool: (t: Tool) => void;
  className: string;
}) {
  const name = m["shell.tool"]({ name: TOOL_NAME[tool]() });
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={name}
          title={name}
          className={`${className} [&_svg]:size-3.5`}
        >
          <ToolGlyph tool={tool} />
          <ChevronDown />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-[260px]">
        {TOOLS.map((t) => (
          <DropdownMenuItem key={t} onSelect={() => onTool(t)}>
            <span className="grid size-5 place-items-center text-primary-text">
              <ToolGlyph tool={t} />
            </span>
            {TOOL_NAME[t]()}
            {t === tool && <Check className="ml-auto size-5 text-lavender" />}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
