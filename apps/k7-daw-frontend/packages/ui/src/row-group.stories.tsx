import type { Meta, StoryObj } from "@storybook/react-vite";
import { Badge } from "./badge";
import { Download, FileAudio, Settings } from "./icons";
import { Row, RowButton, RowGroup } from "./row-group";
import { Switch } from "./switch";

const meta = {
  title: "UI/RowGroup",
  component: RowGroup,
  tags: ["autodocs"],
} satisfies Meta<typeof RowGroup>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Rows at rest, with a disclosure chevron, selected, disabled, 52 px tall, and with trailing controls. */
export const States: Story = {
  render: () => (
    <RowGroup className="w-[360px]">
      <Row icon={<Settings />} detail="Sample rate, buffer">
        Audio device
      </Row>
      <RowButton icon={<Settings />} disclosure>
        Disclosure row
      </RowButton>
      <RowButton icon={<FileAudio />} selected>
        Selected row
      </RowButton>
      <RowButton icon={<FileAudio />} disabled>
        Disabled row
      </RowButton>
      <RowButton tall icon={<Download />} detail="44.1 kHz, 16 bit" disclosure>
        Export MP3
      </RowButton>
      <Row trailing={<Switch defaultChecked />}>Metronome</Row>
      <Row trailing={<Badge variant="count">2</Badge>}>Conflicts</Row>
    </RowGroup>
  ),
};
