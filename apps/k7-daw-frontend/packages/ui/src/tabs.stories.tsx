import type { Meta, StoryObj } from "@storybook/react-vite";
import { Layers, TriangleAlert } from "./icons";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./tabs";

const meta = {
  title: "UI/Tabs",
  component: Tabs,
  tags: ["autodocs"],
} satisfies Meta<typeof Tabs>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <Tabs defaultValue="lyrics">
      <TabsList>
        <TabsTrigger value="lyrics">Lyrics</TabsTrigger>
        <TabsTrigger value="tablature">Tablature</TabsTrigger>
        <TabsTrigger value="sheet">Sheet</TabsTrigger>
      </TabsList>
      <TabsContent value="lyrics">Lyrics content</TabsContent>
      <TabsContent value="tablature">Tablature content</TabsContent>
      <TabsContent value="sheet">Sheet content</TabsContent>
    </Tabs>
  ),
};

/** A tab may carry an icon or a mono count, amber while there are problems. */
export const WithIconAndCount: Story = {
  render: () => (
    <Tabs defaultValue="all">
      <TabsList>
        <TabsTrigger value="all">
          <Layers className="size-4" />
          All
        </TabsTrigger>
        <TabsTrigger value="problems">
          <TriangleAlert className="size-4 text-warn" />
          Problems
          <span className="type-mono-value text-warn">3</span>
        </TabsTrigger>
      </TabsList>
    </Tabs>
  ),
};

export const Disabled: Story = {
  render: () => (
    <Tabs defaultValue="a">
      <TabsList>
        <TabsTrigger value="a">Edits</TabsTrigger>
        <TabsTrigger value="b" disabled>
          Commits
        </TabsTrigger>
      </TabsList>
    </Tabs>
  ),
};
