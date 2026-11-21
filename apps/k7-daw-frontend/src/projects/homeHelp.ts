// homeHelp.ts — how a home page asks for the documentation: HomeFrame provides the opener, anything below it calls it.
// Its own file so the command bar and the menus read it without importing the frame that mounts them.

import { createContext, useContext } from "react";

export const HelpContext = createContext<() => void>(() => {});

/** Opens the documentation over the home page. */
export const useHomeHelp = (): (() => void) => useContext(HelpContext);
