import type { Meta, StoryObj } from "@storybook/react-vite";
import { Switch } from "./switch";

const meta = {
  title: "UI/Switch",
  component: Switch,
  tags: ["autodocs"],
} satisfies Meta<typeof Switch>;

export default meta;
type Story = StoryObj<typeof meta>;

export const On: Story = { args: { defaultChecked: true } };

export const Off: Story = { args: { defaultChecked: false } };

export const Disabled: Story = { args: { disabled: true } };

export const Labelled: Story = {
  render: () => (
    <div className="flex items-center gap-3">
      <Switch id="metronome" defaultChecked />
      <label htmlFor="metronome">Metronome</label>
    </div>
  ),
};
