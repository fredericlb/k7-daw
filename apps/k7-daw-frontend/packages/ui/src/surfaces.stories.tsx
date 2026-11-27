import type { Meta, StoryObj } from "@storybook/react-vite";
import { Card, type CardVariant } from "./card";

const meta = {
  title: "UI/Surfaces",
  component: Card,
  tags: ["autodocs"],
} satisfies Meta<typeof Card>;

export default meta;
type Story = StoryObj<typeof meta>;

const KINDS: [CardVariant, string][] = [
  ["glass", "Glass: header controls, no blur"],
  ["float", "Float: bar, toasts, tab bar"],
  ["panel", "Panel: right panels 0.92"],
  ["dialog", "Dialog and command bar 0.94"],
  ["sheet", "Phone sheet 0.97"],
  ["menu", "Menu 0.9"],
  ["row", "Row group"],
  ["quiet", "Quiet specimen card"],
];

/** Every surface of the canvas over the ground, with the decor behind to show what blurs. */
export const All: Story = {
  render: () => (
    <div className="relative grid grid-cols-2 gap-4 bg-ground p-6">
      <div className="pointer-events-none absolute -top-10 left-10 size-64 rounded-full bg-(--k7-blob-a) opacity-50 blur-[70px]" />
      {KINDS.map(([variant, text]) => (
        <Card key={variant} variant={variant} className="relative">
          <p className="type-label text-fg-1">{text}</p>
        </Card>
      ))}
    </div>
  ),
};
