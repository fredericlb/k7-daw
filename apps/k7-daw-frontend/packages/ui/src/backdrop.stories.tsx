import type { Meta, StoryObj } from "@storybook/react-vite";
import { Backdrop } from "./backdrop";

const meta = {
  title: "UI/Backdrop",
  component: Backdrop,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Backdrop>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <div className="relative h-[480px] overflow-hidden bg-ground">
      <Backdrop />
    </div>
  ),
};
