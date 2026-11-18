// GitContext.tsx — the transport the home page's git screens call through, set once by the pane that holds
// the account's file service, so no prop crosses the table's rows to reach a menu.

import { createContext, useContext } from "react";
import type { GitTransport } from "./gitApi";

const Ctx = createContext<GitTransport | null>(null);

export const GitProvider = Ctx.Provider;
export const useGitTransport = (): GitTransport | null => useContext(Ctx);
