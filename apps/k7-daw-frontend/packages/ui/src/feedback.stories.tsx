import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "./button";
import { EmptyState } from "./empty-state";
import { ErrorCard } from "./error-card";
import { FolderOpen, Music } from "./icons";
import { Progress } from "./progress";
import { Spinner } from "./spinner";

const meta = {
  title: "UI/Feedback",
  tags: ["autodocs"],
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Spinning: Story = {
  render: () => (
    <div className="flex items-center gap-4">
      <Spinner />
      <Spinner className="size-[22px]" label="Loading" />
    </div>
  ),
};

/** Determinate bars at the start, the middle and the end. */
export const ProgressBar: Story = {
  render: () => (
    <div className="flex w-72 flex-col gap-4">
      {[0, 34, 100].map((v) => (
        <Progress key={v} value={v} label={`${v} percent`} />
      ))}
    </div>
  ),
};

/** The page empty state (76 px glyph) and the panel one (64 px, 18 px title). */
export const Empty: Story = {
  render: () => (
    <div className="flex w-[420px] flex-col gap-6">
      <EmptyState
        glyph={<FolderOpen />}
        title="No project yet"
        actions={<Button>New project</Button>}
      >
        Start one, or clone a workspace from git.
      </EmptyState>
      <EmptyState compact glyph={<Music />} title="Select a clip">
        Its events open here.
      </EmptyState>
    </div>
  ),
};

export const Failure: Story = {
  render: () => (
    <ErrorCard title="This module could not load">
      The file is missing from the library.
    </ErrorCard>
  ),
};
