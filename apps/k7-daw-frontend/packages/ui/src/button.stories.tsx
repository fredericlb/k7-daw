import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "./button";
import { Play, SquareTerminal } from "./icons";

const meta = {
  title: "UI/Button",
  component: Button,
  tags: ["autodocs"],
  argTypes: {
    variant: {
      control: "select",
      options: [
        "default",
        "secondary",
        "outline",
        "ghost",
        "glass",
        "chip",
        "destructive",
        "link",
      ],
    },
    size: {
      control: "select",
      options: ["default", "sm", "lg", "icon", "round"],
    },
    disabled: { control: "boolean" },
  },
  args: {
    children: "Button",
    onClick: () => {},
  },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Secondary: Story = {
  args: { variant: "secondary" },
};

export const Outline: Story = {
  args: { variant: "outline" },
};

export const Ghost: Story = {
  args: { variant: "ghost" },
};

export const Destructive: Story = {
  args: { variant: "destructive" },
};

export const Link: Story = {
  args: { variant: "link", children: "Link button" },
};

export const Small: Story = {
  args: { size: "sm" },
};

export const Block: Story = {
  args: { className: "w-full" },
};

export const Disabled: Story = {
  args: { disabled: true },
};

/** The 44 px round glass button, at rest, pressed and disabled. */
export const Round: Story = {
  render: () => (
    <div className="flex items-center gap-3">
      <Button variant="glass" size="round" aria-label="Command bar">
        <SquareTerminal className="size-5" />
      </Button>
      <Button variant="glass" size="round" aria-pressed aria-label="Pressed">
        <SquareTerminal className="size-5" />
      </Button>
      <Button variant="glass" size="round" disabled aria-label="Disabled">
        <SquareTerminal className="size-5" />
      </Button>
      <Button size="round" aria-label="Play">
        <Play className="size-5" />
      </Button>
      <Button size="round" disabled aria-label="Play, disabled">
        <Play className="size-5" />
      </Button>
    </div>
  ),
};

/** Label pills: 44 tall, 36 inside cards and toasts, the chip toggle in both states. */
export const Pills: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-3">
      <Button>Primary</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="destructive">Delete</Button>
      <Button size="sm">Pill 36</Button>
      <Button size="sm" variant="chip">
        Loop
      </Button>
      <Button size="sm" variant="chip" aria-pressed>
        Loop on
      </Button>
      <Button disabled>Disabled</Button>
    </div>
  ),
};
