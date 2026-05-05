import SessionList from "./SessionList";
import TaskInput from "./TaskInput";
import Results from "./Results";
import LogPane from "./LogPane";

export default function Workbench() {
  return (
    <div className="workbench">
      <section className="pane pane-sessions">
        <div className="pane-header">Sessions</div>
        <div className="pane-body">
          <SessionList />
        </div>
      </section>
      <section className="pane pane-task">
        <div className="pane-header">Task</div>
        <div className="pane-body">
          <TaskInput />
        </div>
      </section>
      <section className="pane pane-results">
        <div className="pane-header">Results</div>
        <div className="pane-body">
          <Results />
        </div>
      </section>
      <section className="pane pane-logs">
        <div className="pane-header">Logs</div>
        <div className="pane-body">
          <LogPane />
        </div>
      </section>
    </div>
  );
}
