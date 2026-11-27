// Needs `@source` on this package's src in .storybook/preview.css, or the toast draws unstyled.

import type { Meta, StoryObj } from "@storybook/react-vite";
import { Unplug } from "@studio/ui/icons";
import { useEffect } from "react";
import {
  dismissToast,
  showToast,
  Toaster,
  type ToastPlacement,
  updateToast,
} from "./toast";

const meta = {
  title: "Shell/Toast",
  component: Toaster,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Toaster>;

export default meta;
type Story = StoryObj<typeof meta>;

function Stage({
  show,
  phone,
}: {
  show: () => (() => void) | undefined;
  phone?: boolean;
}) {
  useEffect(() => show(), [show]);
  return (
    <div className="relative h-[560px] bg-ground">
      {<Toaster phone={phone} />}
    </div>
  );
}

const still =
  (placement: ToastPlacement = "bottom-right") =>
  () => {
    showToast({ message: "Saved", duration: 0, placement });
    return undefined;
  };

export const Success: Story = {
  render: () => <Stage show={still()} />,
};

export const Warning: Story = {
  render: () => (
    <Stage
      show={() => {
        showToast({
          variant: "warning",
          title: "Sample missing",
          message: "kick.wav is not in the workspace.",
          duration: 0,
        });
        return undefined;
      }}
    />
  ),
};

/** An error with an action pill; it stays until dismissed. */
export const ErrorWithAction: Story = {
  render: () => (
    <Stage
      show={() => {
        showToast({
          variant: "error",
          title: "Engine lost",
          message: "The audio engine stopped answering.",
          icon: <Unplug />,
          action: { label: "Restart", onClick: () => {} },
        });
        return undefined;
      }}
    />
  ),
};

/** The plugin-scan toast: neutral, a spinner, a count and a bar, no Dismiss, top-centre. */
export const ScanProgress: Story = {
  render: () => (
    <Stage
      show={() => {
        const id = showToast({
          variant: "neutral",
          placement: "top-centre",
          title: "Scanning plugins",
          message: "Looking in the plugin folders",
          progress: { value: 34, total: 120 },
        });
        const timer = setInterval(
          () =>
            updateToast(id, {
              progress: { value: 35 + Math.random() * 80, total: 120 },
            }),
          1200,
        );
        return () => {
          clearInterval(timer);
          dismissToast(id);
        };
      }}
    />
  ),
};

/** The phone stacks: bottom toasts 140 px above the edge, the scan toast at the top. */
export const Phone: Story = {
  render: () => (
    <div className="mx-auto w-[390px]">
      <Stage
        phone
        show={() => {
          showToast({ message: "Pulled and merged.", duration: 0 });
          showToast({
            variant: "neutral",
            placement: "top-centre",
            message: "Scanning plugins",
            progress: { value: 12, total: 120 },
          });
          return undefined;
        }}
      />
    </div>
  ),
};
