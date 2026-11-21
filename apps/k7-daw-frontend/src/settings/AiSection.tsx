// AiSection.tsx — the account's Claude key and model, which the module builder's "AI organize"
// runs on. The key is stored by k7's backend for this account and is WRITE-ONLY from here:
// what comes back is a hint of it, never the key. See ../../docs/settings.md.

import { askConfirm } from "@studio/project-shell/ask";
import { TriangleAlert } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { useEffect, useState } from "react";
import { type AiSettingsView, aiOrganize } from "../api";
import { m } from "../paraglide/messages.js";
import { Action, Row, Section } from "./Section";

export function AiSection() {
  const [view, setView] = useState<AiSettingsView | null>(null);
  const [key, setKey] = useState("");
  const [model, setModel] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void aiOrganize
      .settings()
      .then((v) => {
        setView(v);
        setModel(v.model ?? "");
      })
      .catch((e: Error) => setError(e.message));
  }, []);

  const save = (patch: { apiKey?: string | null; model?: string | null }) => {
    setBusy(true);
    setError(null);
    void aiOrganize
      .saveSettings(patch)
      .then((v) => {
        setView(v);
        setModel(v.model ?? "");
        // The typed key is gone from the page once stored: the hint is all that stays on screen.
        if (patch.apiKey !== undefined) setKey("");
      })
      .catch((e: Error) => setError(e.message))
      .finally(() => setBusy(false));
  };

  // The key is never shown again, so removing it is asked first.
  const removeKey = async () => {
    const ok = await askConfirm({
      title: m["ai.removeTitle"](),
      body: m["ai.removeBody"](),
      confirmLabel: m["ai.remove"](),
      danger: true,
    });
    if (ok) save({ apiKey: null });
  };

  const modelChanged = model.trim() !== (view?.model ?? "");

  return (
    <Section title={m["ai.title"]()} hint={m["ai.hint"]()}>
      <Row
        label={
          view?.keyHint
            ? m["ai.keyHint"]({ hint: view.keyHint })
            : m["ai.noKey"]()
        }
        detail={view?.keyHint ? m["ai.keyOn"]() : m["ai.keyOff"]()}
      >
        {view?.keyHint && (
          <Action danger disabled={busy} onClick={() => void removeKey()}>
            {m["ai.remove"]()}
          </Action>
        )}
      </Row>
      <div className="flex gap-2 px-3.5 py-2.5">
        <Input
          type="password"
          autoComplete="off"
          aria-label={m["ai.newKey"]()}
          placeholder={view?.keyHint ? m["ai.replaceKey"]() : "sk-ant-…"}
          value={key}
          onChange={(e) => setKey(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && key.trim()) save({ apiKey: key });
          }}
        />
        <Action
          disabled={busy || !key.trim()}
          onClick={() => save({ apiKey: key })}
        >
          {m["common.save"]()}
        </Action>
      </div>
      <div className="flex gap-2 px-3.5 py-2.5">
        <Input
          aria-label={m["ai.model"]()}
          placeholder={view?.defaultModel ?? m["ai.modelPlaceholder"]()}
          title={m["ai.modelHint"]()}
          value={model}
          onChange={(e) => setModel(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && modelChanged)
              save({ model: model || null });
          }}
        />
        <Action
          disabled={busy || !modelChanged}
          onClick={() => save({ model: model || null })}
        >
          {m["common.save"]()}
        </Action>
      </div>
      {error && (
        <div
          role="alert"
          className="type-secondary flex items-start gap-2 px-3.5 py-2.5 text-error"
        >
          <TriangleAlert className="mt-0.5 size-5 shrink-0" aria-hidden />
          <span className="min-w-0 break-words">{error}</span>
        </div>
      )}
    </Section>
  );
}
