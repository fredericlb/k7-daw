import type { Meta, StoryObj } from "@storybook/react-vite";
import { Badge } from "./badge";
import { Capsule, CapsuleButton, CapsuleDivider } from "./capsule";
import {
  AudioLines,
  ChevronDown,
  FileCode,
  GitCommitHorizontal,
  Workflow,
} from "./icons";

const meta = {
  title: "UI/Capsule",
  component: Capsule,
  tags: ["autodocs"],
} satisfies Meta<typeof Capsule>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The pane switcher: the current pane shows its label (pressed), the others are icon-only. */
export const PaneSwitcher: Story = {
  render: () => (
    <Capsule aria-label="Panes">
      <CapsuleButton pressed aria-label="Arrangement">
        <AudioLines />
        Arrangement
      </CapsuleButton>
      <CapsuleButton pressed={false} aria-label="Graph">
        <Workflow />
      </CapsuleButton>
      <CapsuleButton pressed={false} aria-label="Source">
        <FileCode />
      </CapsuleButton>
    </Capsule>
  ),
};

/** The git capsule: a labelled action with a count, a divider, and a menu chevron; dimmed with nothing to commit. */
export const Git: Story = {
  render: () => (
    <div className="flex gap-4">
      <Capsule aria-label="Git">
        <CapsuleButton>
          <GitCommitHorizontal />
          Commit &amp; Push
          <Badge variant="count">3</Badge>
        </CapsuleButton>
        <CapsuleDivider />
        <CapsuleButton aria-label="Git menu">
          <ChevronDown />
        </CapsuleButton>
      </Capsule>
      <Capsule aria-label="Git, nothing to commit">
        <CapsuleButton disabled>
          <GitCommitHorizontal />
          Commit &amp; Push
        </CapsuleButton>
        <CapsuleDivider />
        <CapsuleButton aria-label="Git menu">
          <ChevronDown />
        </CapsuleButton>
      </Capsule>
    </div>
  ),
};
