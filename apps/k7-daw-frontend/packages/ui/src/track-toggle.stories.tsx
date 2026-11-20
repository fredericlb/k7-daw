import type { Meta, StoryObj } from "@storybook/react-vite";
import { TrackToggle } from "./track-toggle";

const meta = {
  title: "UI/TrackToggle",
  component: TrackToggle,
  tags: ["autodocs"],
} satisfies Meta<typeof TrackToggle>;

export default meta;
type Story = StoryObj<typeof meta>;

/** M, S and A, each off and on; one component for the lane header and the mixer strip. */
export const States: Story = {
  args: { kind: "mute", "aria-label": "Mute" },
  render: () => (
    <div className="flex flex-col gap-3">
      <div className="flex gap-2">
        <TrackToggle kind="mute" aria-label="Mute" />
        <TrackToggle kind="solo" aria-label="Solo" />
        <TrackToggle kind="arm" aria-label="Arm" />
      </div>
      <div className="flex gap-2">
        <TrackToggle kind="mute" on aria-label="Mute, on" />
        <TrackToggle kind="solo" on aria-label="Solo, on" />
        <TrackToggle kind="arm" on aria-label="Arm, on" />
      </div>
    </div>
  ),
};
