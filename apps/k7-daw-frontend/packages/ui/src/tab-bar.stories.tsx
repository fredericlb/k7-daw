import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
  Folder,
  Library,
  ListChecks,
  Snowflake,
  SquareTerminal,
} from "./icons";
import { TabBar, TabBarItem, TabBarRound } from "./tab-bar";

const meta = {
  title: "UI/TabBar",
  component: TabBar,
  tags: ["autodocs"],
} satisfies Meta<typeof TabBar>;

export default meta;
type Story = StoryObj<typeof meta>;

const TABS = [
  ["projects", "Projects", <Folder key="p" />],
  ["tasks", "Tasks", <ListChecks key="t" />],
  ["libraries", "Libraries", <Library key="l" />],
  ["frozen", "Frozen", <Snowflake key="f" />],
] as const;

/** Four tabs and the round command-bar button; the lozenge slides to the selected tab. */
export const Home: Story = {
  args: { label: "Home", children: null },
  render: (args) => {
    const [tab, setTab] = useState("projects");
    return (
      <TabBar
        {...args}
        trailing={
          <TabBarRound aria-label="Command bar">
            <SquareTerminal />
          </TabBarRound>
        }
      >
        {TABS.map(([id, label, icon]) => (
          <TabBarItem
            key={id}
            icon={icon}
            label={label}
            selected={tab === id}
            onClick={() => setTab(id)}
          />
        ))}
      </TabBar>
    );
  },
};
