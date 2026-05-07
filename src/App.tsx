import { useEffect, useState, useSyncExternalStore } from "react";
import { ProjectProvider } from "./lib/projectContext";
import { registerCoreTabs } from "./lib/registerCoreTabs";
import { getSnapshot, subscribe } from "./lib/tabRegistry";
import { tabIdForLocation } from "./lib/hashRoute";
import ProjectTabs from "./components/Workbench/ProjectTabs";
import "./styles/workbench.css";

registerCoreTabs();

function initialTabId(tabs: ReturnType<typeof getSnapshot>): string {
  if (typeof window !== "undefined") {
    const mapped = tabIdForLocation({
      hash: window.location.hash,
      pathname: window.location.pathname,
    });
    if (mapped && tabs.find((t) => t.id === mapped)) return mapped;
  }
  return tabs[0]?.id ?? "workbench";
}

export default function App() {
  const tabs = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const [activeTabId, setActiveTabId] = useState<string>(() => initialTabId(tabs));

  useEffect(() => {
    if (!tabs.find((t) => t.id === activeTabId) && tabs[0]) {
      setActiveTabId(tabs[0].id);
    }
  }, [tabs, activeTabId]);

  useEffect(() => {
    function onHash() {
      const mapped = tabIdForLocation({
        hash: window.location.hash,
        pathname: window.location.pathname,
      });
      if (mapped && tabs.find((t) => t.id === mapped)) setActiveTabId(mapped);
    }
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, [tabs]);

  const ActiveComponent = tabs.find((t) => t.id === activeTabId)?.component;

  return (
    <ProjectProvider>
      <div className="app-shell">
        <header className="app-header">
          <div className="app-title">MoA Desktop</div>
          <ProjectTabs />
          <nav className="app-nav">
            {tabs.map((t) => (
              <button
                key={t.id}
                className={t.id === activeTabId ? "nav-btn active" : "nav-btn"}
                onClick={() => setActiveTabId(t.id)}
              >
                {t.title}
              </button>
            ))}
          </nav>
        </header>
        <main className="app-main">{ActiveComponent ? <ActiveComponent /> : null}</main>
      </div>
    </ProjectProvider>
  );
}
