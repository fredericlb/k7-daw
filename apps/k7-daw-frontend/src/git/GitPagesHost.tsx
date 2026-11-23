// GitPagesHost.tsx — draws the home's git page its address names (`?sync=<workspace>`, `?clone`), over the home.
// Mounted by the pane that holds the account's file service, so the pages call through the same transport as the cards.

import { useSearchParams } from "react-router-dom";
import { ClonePage } from "./ClonePage";
import { CLONE_PARAM, closeGitPage, SYNC_PARAM } from "./gitPages";
import { SyncSettingsPage } from "./SyncSettingsPage";

export function GitPagesHost({ onChanged }: { onChanged: () => void }) {
  const [params] = useSearchParams();
  const sync = params.get(SYNC_PARAM);
  if (sync)
    return (
      <SyncSettingsPage
        key={sync}
        workspace={sync}
        onClose={closeGitPage}
        onConnected={onChanged}
      />
    );
  if (params.has(CLONE_PARAM))
    return <ClonePage onClose={closeGitPage} onCloned={onChanged} />;
  return null;
}
