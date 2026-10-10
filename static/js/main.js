/* Điểm khởi động giao diện: nối tab, tab kiểm tra và tab profile. */

import { $ } from "./utils.js";
import { initTabs } from "./ui/tabs.js";
import { initRun, applyProfile, collectForm, runIfRequested } from "./run.js";
import { initProfiles, activeProfile } from "./profiles.js";

async function main() {
  initTabs((name) => {
    if (name === "profiles") $("profile-search")?.focus({ preventScroll: true });
  });

  initRun({ getProfileId: () => activeProfile()?.id || "" });

  const remembered = await initProfiles({ applyProfile, collectForm });
  if (remembered) applyProfile(remembered);

  runIfRequested();
}

main();
