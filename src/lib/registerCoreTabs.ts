import { registerTab } from "./tabRegistry";
import Workbench from "../components/Workbench/Workbench";
import Settings from "../components/Workbench/Settings";

let done = false;

export function registerCoreTabs(): void {
  if (done) return;
  done = true;
  registerTab({ id: "workbench", title: "Workbench", component: Workbench, order: 0 });
  registerTab({ id: "settings", title: "Settings", component: Settings, order: 100 });
}
