// mountSite.tsx — the entry of the documentation site, k7.rocks/docs: the user and developer guides of docs-site/,
// with no gate and no engine. Not the in-app `help` page, which documents apps/k7-daw-frontend/user-docs/ (../../docs/docsSite.md).

import { clearDevPerfEntries } from "@studio/ui/dev-perf";
import ReactDOM from "react-dom/client";
import { DocsSite } from "./site/DocsSite";
import "../index.css";

clearDevPerfEntries(import.meta.env.DEV);

const root = document.getElementById("root");
if (root)
  ReactDOM.createRoot(root).render(
    <DocsSite homeUrl={`${window.location.origin}/`} />,
  );
