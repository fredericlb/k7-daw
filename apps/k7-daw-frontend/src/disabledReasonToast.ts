// disabledReasonToast.ts — wires the kit's disabled-reason hook to the shell's toast; imported once by appMain.
// A reason is a sentence the person must be able to read, so it is a warning toast, never a hover title.

import { showToast } from "@studio/project-shell/toast";
import { setDisabledReasonSink } from "@studio/ui/disabled-reason";

setDisabledReasonSink((message) =>
  showToast({ message, variant: "warning", duration: 4000 }),
);
