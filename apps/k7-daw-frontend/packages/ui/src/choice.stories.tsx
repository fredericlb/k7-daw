import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Choice } from "./choice";

const meta = {
  title: "UI/Choice",
  component: Choice,
  tags: ["autodocs"],
} satisfies Meta<typeof Choice>;

export default meta;
type Story = StoryObj<typeof meta>;

const OPTIONS = [
  { value: "https", label: "HTTPS" },
  { value: "ssh", label: "SSH" },
];

/** Segmented on Pad and Desk, radio rows on Phone. */
export const Layouts: Story = {
  args: {
    label: "Transport",
    options: OPTIONS,
    value: "https",
    onValueChange: () => {},
  },
  render: () => {
    const [v, setV] = useState("https");
    return (
      <div className="flex w-72 flex-col gap-6">
        <Choice
          label="Transport"
          options={OPTIONS}
          value={v}
          onValueChange={setV}
        />
        <Choice
          label="Transport"
          layout="rows"
          options={OPTIONS}
          value={v}
          onValueChange={setV}
        />
      </div>
    );
  },
};
