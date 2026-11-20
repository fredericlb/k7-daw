import type { Meta, StoryObj } from "@storybook/react-vite";
import * as React from "react";
import { Button } from "./button";
import { Mic, Plus } from "./icons";
import { RowButton, RowGroup } from "./row-group";
import { Sheet, SidePanel } from "./side-panel";

const meta = {
  title: "UI/SidePanel",
  component: SidePanel,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof SidePanel>;

export default meta;
type Story = StoryObj<typeof meta>;

const rows = (
  <RowGroup>
    <RowButton>One</RowButton>
    <RowButton selected>Two</RowButton>
    <RowButton>Three</RowButton>
  </RowGroup>
);

/** The 400 px floating panel (Pad and Desk). */
export const Panel: Story = {
  args: {
    title: "Resources",
    closeLabel: "Close",
    onClose: () => {},
    children: null,
  },
  render: (args) => (
    <div className="relative h-[560px] bg-background">
      <SidePanel {...args} icon={<Mic />}>
        {rows}
      </SidePanel>
    </div>
  ),
};

/** An editor panel: the event's name over what it is, with an action before Close. */
export const EditorHeader: Story = {
  args: {
    title: "melody",
    closeLabel: "Close",
    onClose: () => {},
    children: null,
  },
  render: (args) => (
    <div className="relative h-[560px] bg-background">
      <SidePanel
        {...args}
        subtitle="audio · bar 2 · 0:00.50 → 0:01.75"
        icon={<Mic />}
        actions={
          <Button variant="glass" size="round" aria-label="Add">
            <Plus className="size-5" />
          </Button>
        }
      >
        {rows}
      </SidePanel>
    </div>
  ),
};

/** The phone sheet, with an editor's own transport row. */
export const PhoneSheet: Story = {
  args: {
    title: "Events",
    closeLabel: "Close",
    onClose: () => {},
    children: null,
  },
  render: (args) => (
    <div className="relative mx-auto h-[720px] w-[390px] overflow-hidden bg-background">
      <Sheet
        {...args}
        transport={
          <div className="glass type-mono-value rounded-full px-4 py-2 text-fg-2">
            ▶ 0:01.25
          </div>
        }
      >
        {rows}
      </Sheet>
    </div>
  ),
};

/** Escape and Close both end it; the panes stay clickable while a SidePanel is up. */
export const Closable: Story = {
  args: {
    title: "Resources",
    closeLabel: "Close",
    onClose: () => {},
    children: null,
  },
  render: (args) => {
    const [open, setOpen] = React.useState(true);
    return (
      <div className="relative h-[560px] bg-background p-4">
        <Button onClick={() => setOpen(true)}>Open panel</Button>
        {open ? (
          <SidePanel {...args} icon={<Mic />} onClose={() => setOpen(false)}>
            {rows}
          </SidePanel>
        ) : null}
      </div>
    );
  },
};
