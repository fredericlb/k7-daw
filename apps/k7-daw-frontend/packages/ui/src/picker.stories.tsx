import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Picker } from "./picker";

const meta = {
  title: "UI/Picker",
  component: Picker,
  tags: ["autodocs"],
} satisfies Meta<typeof Picker>;

export default meta;
type Story = StoryObj<typeof meta>;

const ITEMS = [
  { value: "a", label: "Warm room", detail: "reverbs/warm-room.k7x" },
  { value: "b", label: "Tape wobble", detail: "mods/tape-wobble.k7x" },
  { value: "c", label: "Plate", detail: "reverbs/plate.k7x" },
];

/** Closed it shows the current item and its path; click it to search the list. */
export const Default: Story = {
  args: {
    items: ITEMS,
    value: "a",
    label: "Module",
    searchLabel: "Search",
    emptyLabel: "Nothing matches",
    onValueChange: () => {},
  },
  render: (args) => {
    const [v, setV] = useState<string | null>("a");
    return (
      <div className="w-80">
        <Picker {...args} value={v} onValueChange={setV} />
      </div>
    );
  },
};

export const Empty: Story = {
  args: { ...Default.args, value: null, placeholder: "Choose a module" },
  render: Default.render,
};
