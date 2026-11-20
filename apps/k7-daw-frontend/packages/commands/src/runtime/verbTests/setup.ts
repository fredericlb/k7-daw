// setup.ts — before each verb test under the shims: commands.yaml is handed in, as the Worker's bundle hands it in.
// What a reader gets wrong: without it the catalogue is read through `node:fs`, which is the empty Vfs here.

import { provideCommandsYaml } from "../../commandSpec.ts";
import { commandsYamlText } from "../realFs.ts";

provideCommandsYaml(commandsYamlText());
