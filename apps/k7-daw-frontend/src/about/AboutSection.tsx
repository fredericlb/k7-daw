// AboutSection.tsx — About, inside App settings: k7's name and version, its licence with the full text, the link to its
// source, and every third-party notice. The source link is not courtesy: the engine links JUCE under
// the AGPLv3, so a network user must be offered the source. See ../../docs/about.md.

import { buttonVariants } from "@studio/ui/button";
import { ExternalLink } from "@studio/ui/icons";
import { lazy, Suspense, useState } from "react";
import { useLocalApp } from "../localApp";
import { m } from "../paraglide/messages.js";
import { Action, Row, Section } from "../settings/Section";
import { VERSION } from "../version";
import { LicenseText } from "./LicenseText";

/** Where the complete source is published: the address the AGPL's offer names. */
export const SOURCE_URL = "https://github.com/fredericlb/k7-daw";

/** The notices and their texts, a chunk of their own that only this page fetches. */
const NoticesSection = lazy(() =>
  import("./NoticesSection").then((x) => ({ default: x.NoticesSection })),
);

/** k7's own licence, the root LICENSE, fetched with the notices since it is one of their texts. */
const OwnLicense = lazy(() =>
  import("virtual:k7-notices").then((x) => ({
    default: () => <LicenseText text={x.default.standards["GPL-3.0"] ?? ""} />,
  })),
);

export function AboutSection() {
  const [showLicense, setShowLicense] = useState(false);
  const local = useLocalApp();
  return (
    <div role="region" aria-label={m["about.title"]()} className="flex flex-col gap-4">
        <Section title="K7 DAW" hint={m["about.tagline"]()}>
          <Row
            // version.ts is the frontend's one version source, whatever feeds it (../../docs/about.md).
            label={m["about.version"]({ version: VERSION })}
            detail={
              local?.engine
                ? m["settings.desktopVersion"]({ version: local.version })
                : undefined
            }
          />
        </Section>

        <Section title={m["about.license.title"]()}>
          <Row label={m["about.license.body"]()}>
            <Action onClick={() => setShowLicense((s) => !s)}>
              {showLicense
                ? m["about.license.hide"]()
                : m["about.license.show"]()}
            </Action>
          </Row>
          {showLicense && (
            <Suspense fallback={null}>
              <OwnLicense />
            </Suspense>
          )}
        </Section>

        <Section
          title={m["about.source.title"]()}
          hint={m["about.source.hint"]()}
        >
          <Row label={SOURCE_URL}>
            <a
              href={SOURCE_URL}
              target="_blank"
              rel="noreferrer"
              className={buttonVariants({ variant: "glass", size: "sm" })}
            >
              <ExternalLink className="size-5" />
              {m["about.source.open"]()}
            </a>
          </Row>
        </Section>

        <Suspense
          fallback={
            <p className="type-secondary text-fg-2">
              {m["about.notices.loading"]()}
            </p>
          }
        >
          <NoticesSection />
        </Suspense>
    </div>
  );
}
