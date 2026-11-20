import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Knob } from "./knob";

const meta = {
  title: "UI/Knob",
  component: Knob,
  tags: ["autodocs"],
} satisfies Meta<typeof Knob>;

export default meta;
type Story = StoryObj<typeof meta>;

const db = (v: number) => `${(v * 24 - 12).toFixed(1)} dB`;

/** A 40 px knob, a 36 and a 44, and the peak chip at rest and clipping. */
export const Sizes: Story = {
  args: { value: 0.5, label: "Gain", onValueChange: () => {} },
  render: () => {
    const [v, setV] = useState(0.5);
    return (
      <div className="flex gap-6">
        {[36, 40, 44].map((size) => (
          <Knob
            key={size}
            label={`Gain ${size}`}
            size={size}
            value={v}
            format={db}
            onValueChange={setV}
          />
        ))}
        <Knob
          label="Level"
          value={v}
          format={db}
          peak="-1.2"
          onValueChange={setV}
        />
        <Knob
          label="Level, clipping"
          value={0.9}
          format={db}
          peak="+0.4"
          clipping
          onValueChange={setV}
        />
      </div>
    );
  },
};
