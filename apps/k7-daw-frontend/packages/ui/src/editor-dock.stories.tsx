import type { Meta, StoryObj } from "@storybook/react-vite";
import * as React from "react";
import { EditorDock, EditorSheet } from "./editor-dock";
import { Music } from "./icons";

const meta = {
  title: "UI/EditorDock",
  component: EditorDock,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof EditorDock>;

export default meta;
type Story = StoryObj<typeof meta>;

const labels = {
  title: "Events",
  subtitle: "kit take · bar 2",
  closeLabel: "Close editor",
  expandLabel: "Expand editor",
  resizeLabel: "Resize the editor: drag up or down",
  onClose: () => {},
  children: null,
};

/** The bottom dock (Pad and Desk): drag the grabber, or use the arrow keys on it; Expand takes it under the header. */
export const Dock: Story = {
  args: {
    ...labels,
    height: 380,
    onHeightChange: () => {},
    expanded: false,
    onExpandedChange: () => {},
  },
  render: (args) => {
    const [height, setHeight] = React.useState(380);
    const [expanded, setExpanded] = React.useState(false);
    return (
      <div className="relative h-[820px] bg-ground">
        <EditorDock
          {...args}
          icon={<Music />}
          height={height}
          onHeightChange={setHeight}
          expanded={expanded}
          onExpandedChange={setExpanded}
        />
      </div>
    );
  },
};

/** The phone sheet: partial and non-modal, full screen and modal once expanded. */
export const Sheet: Story = {
  args: Dock.args,
  render: (args) => {
    const [expanded, setExpanded] = React.useState(false);
    return (
      <div className="relative h-[844px] bg-ground">
        <EditorSheet
          {...args}
          icon={<Music />}
          expanded={expanded}
          onExpandedChange={setExpanded}
        />
      </div>
    );
  },
};
