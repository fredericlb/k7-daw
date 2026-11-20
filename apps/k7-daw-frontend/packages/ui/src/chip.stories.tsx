import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Chip } from "./chip";

const meta = {
  title: "UI/Chip",
  component: Chip,
  tags: ["autodocs"],
} satisfies Meta<typeof Chip>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Anchor: Story = {
  render: () => (
    <div className="flex gap-2">
      <Chip variant="anchor">warm-bus</Chip>
      <Chip variant="anchor" orphan>
        lost-chain
      </Chip>
    </div>
  ),
};

export const Versions: Story = {
  render: () => (
    <div className="flex gap-2">
      <Chip variant="version">v4</Chip>
      <Chip variant="version">v5</Chip>
      <Chip variant="version" newest>
        v6 · newest
      </Chip>
    </div>
  ),
};

export const Filters: Story = {
  render: () => {
    const [on, setOn] = useState("all");
    return (
      <div className="flex gap-2">
        {["all", "audio", "video", "records"].map((f) => (
          <Chip
            key={f}
            variant="filter"
            selected={on === f}
            onClick={() => setOn(f)}
          >
            {f}
          </Chip>
        ))}
      </div>
    );
  },
};

export const GitStatus: Story = {
  render: () => (
    <div className="flex gap-2">
      <Chip variant="status" tone="new">
        New
      </Chip>
      <Chip variant="status" tone="changed">
        Changed
      </Chip>
      <Chip variant="status" tone="removed">
        Removed
      </Chip>
      <Chip variant="status" tone="renamed">
        Renamed
      </Chip>
      <Chip variant="status" tone="choose">
        To choose
      </Chip>
    </div>
  ),
};
