// project.spec.ts — each seeded example project opens into its editor on the machine's engine.
// It needs an engine the account can reach; without one the picker is all a project shows (docs/e2e.md).
import { projectLink } from "../../src/home.ts";
import { expect, test } from "../../src/target.ts";
import { engineUp, transportBar } from "../../src/transport.ts";
import { watchPage } from "../../src/watch.ts";

// One demo, the one with a picture: the golden suite renders all five and judges their sound.
const EXAMPLES = ["video-demo"];

for (const name of EXAMPLES) {
  test(`${name} opens on the engine`, async ({ k7 }) => {
    // The home page folds every workspace; the address unfolds the one whose projects are links (apps/k7-daw-frontend/docs/projects.md).
    const home = await k7.open("/?ws=Examples");
    // The app cancels the link's navigation and opens a tab instead, so the click must not wait for one.
    await projectLink(home, "Examples", name).click({ noWaitAfter: true });
    const page = await k7.landed(home, new RegExp(`/@local/Examples/${name}`));
    const watch = watchPage(page);
    await engineUp(page);
    await expect(
      transportBar(page).getByRole("button", { name: "Play", exact: true }),
    ).toBeEnabled();
    await page.screenshot({
      path: `test-results/projects/${k7.target}-${name}.png`,
    });
    watch.assertClean();
  });
}
