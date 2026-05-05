import { useProject } from "../../lib/projectContext";

export default function ProjectTabs() {
  const { projects, active, setActive, addProject } = useProject();

  return (
    <div className="project-tabs">
      {projects.map((p) => (
        <button
          key={p.id}
          className={p.id === active.id ? "tab active" : "tab"}
          onClick={() => setActive(p.id)}
          title={p.repoPath ?? p.title}
        >
          {p.title}
        </button>
      ))}
      <button
        className="tab tab-add"
        title="Add project (stub)"
        onClick={() => {
          const n = projects.length + 1;
          addProject({ id: `p${n}`, title: `project ${n}` });
        }}
      >
        +
      </button>
    </div>
  );
}
