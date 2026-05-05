import { createContext, useContext, useMemo, useState, type ReactNode } from "react";

export type ProjectId = string;

export interface ProjectInfo {
  id: ProjectId;
  title: string;
  repoPath?: string;
}

interface ProjectContextValue {
  active: ProjectInfo;
  projects: ProjectInfo[];
  setActive: (id: ProjectId) => void;
  addProject: (p: ProjectInfo) => void;
}

const defaultProject: ProjectInfo = {
  id: "default",
  title: "default project",
};

const Ctx = createContext<ProjectContextValue | null>(null);

export function ProjectProvider({ children }: { children: ReactNode }) {
  const [projects, setProjects] = useState<ProjectInfo[]>([defaultProject]);
  const [activeId, setActiveId] = useState<ProjectId>(defaultProject.id);

  const value = useMemo<ProjectContextValue>(
    () => ({
      active: projects.find((p) => p.id === activeId) ?? defaultProject,
      projects,
      setActive: (id) => setActiveId(id),
      addProject: (p) => setProjects((cur) => (cur.some((c) => c.id === p.id) ? cur : [...cur, p])),
    }),
    [projects, activeId]
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useProject(): ProjectContextValue {
  const v = useContext(Ctx);
  if (!v) throw new Error("useProject must be used within ProjectProvider");
  return v;
}
