import { registerTab } from "./tabRegistry";
import Workbench from "../components/Workbench/Workbench";
import Settings from "../components/Workbench/Settings";
import SynthViewDemo from "../dev/SynthViewDemo";

let done = false;

export function registerCoreTabs(): void {
  if (done) return;
  done = true;
  registerTab({ id: "workbench", title: "Workbench", component: Workbench, order: 0 });
  registerTab({ id: "settings", title: "Settings", component: Settings, order: 100 });
  if (import.meta.env.DEV) {
    registerTab({
      id: "dev-synthview",
      title: "Dev: SynthView",
      component: SynthViewDemo,
      order: 200,
    });
  }
}
