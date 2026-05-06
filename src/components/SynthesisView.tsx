import { useEffect, useRef, useState } from "react";
import {
  EMPTY_SYNTHESIS,
  SYNTHESIS_COLUMN_LABEL,
  SYNTHESIS_COLUMN_ORDER,
  type SynthesisColumn,
  type SynthesisData,
  type SynthesisRow,
} from "../lib/synthesisTypes";

const NARROW_BREAKPOINT_PX = 720;

export interface SynthesisViewProps {
  data?: SynthesisData;
}

export default function SynthesisView({ data = EMPTY_SYNTHESIS }: SynthesisViewProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [narrow, setNarrow] = useState(false);

  useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setNarrow(entry.contentRect.width < NARROW_BREAKPOINT_PX);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const columns: SynthesisColumn[] = SYNTHESIS_COLUMN_ORDER;
  const total =
    data.verified.length +
    data.claudeOnly.length +
    data.codexOnly.length +
    data.disagreement.length +
    data.open.length;

  return (
    <section className="results-section synthesis-view" ref={containerRef}>
      <h4>Synthesis (5 columns)</h4>
      {total === 0 ? (
        <p className="synthesis-empty">no synthesis rows yet — run a task</p>
      ) : narrow ? (
        <div className="synthesis-accordion">
          {columns.map((col) => (
            <ColumnAccordion key={col} column={col} rows={rowsFor(col, data)} />
          ))}
        </div>
      ) : (
        <div className="synthesis-table" role="table">
          <div className="synthesis-row synthesis-head" role="row">
            {columns.map((col) => (
              <div key={col} className="synthesis-col-header" role="columnheader">
                {SYNTHESIS_COLUMN_LABEL[col]}
                <span className="synthesis-count">{rowsFor(col, data).length}</span>
              </div>
            ))}
          </div>
          <div className="synthesis-row synthesis-body" role="row">
            {columns.map((col) => (
              <div key={col} className="synthesis-col" role="cell">
                <ColumnRows rows={rowsFor(col, data)} />
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function rowsFor(col: SynthesisColumn, data: SynthesisData): SynthesisRow[] {
  switch (col) {
    case "verified":
      return data.verified;
    case "claude_only":
      return data.claudeOnly;
    case "codex_only":
      return data.codexOnly;
    case "disagreement":
      return data.disagreement;
    case "open":
      return data.open;
  }
}

function ColumnAccordion({
  column,
  rows,
}: {
  column: SynthesisColumn;
  rows: SynthesisRow[];
}) {
  return (
    <details className="synthesis-acc" open={rows.length > 0}>
      <summary>
        <span>{SYNTHESIS_COLUMN_LABEL[column]}</span>
        <span className="synthesis-count">{rows.length}</span>
      </summary>
      <ColumnRows rows={rows} />
    </details>
  );
}

function ColumnRows({ rows }: { rows: SynthesisRow[] }) {
  if (rows.length === 0) {
    return <p className="synthesis-col-empty">—</p>;
  }
  return (
    <ul className="synthesis-col-list">
      {rows.map((row, i) => (
        <li key={i} className="synthesis-item">
          <RowBody row={row} />
        </li>
      ))}
    </ul>
  );
}

function RowBody({ row }: { row: SynthesisRow }) {
  switch (row.kind) {
    case "verified":
    case "codex_only":
    case "claude_only":
      return (
        <>
          <div className="synthesis-claim">{row.claim}</div>
          <div className="synthesis-meta">
            <span className={`synthesis-conf conf-${row.confidence}`}>{row.confidence}</span>
            {row.sources.length > 0 ? (
              <span className="synthesis-sources">{row.sources.join(", ")}</span>
            ) : null}
          </div>
          {"note" in row && row.note ? (
            <div className="synthesis-note">{row.note}</div>
          ) : null}
        </>
      );
    case "disagreement":
      return (
        <>
          <div className="synthesis-claim">{row.topic}</div>
          <div className="synthesis-meta">
            <span className="synthesis-pos">Claude: {row.claudePosition}</span>
          </div>
          <div className="synthesis-meta">
            <span className="synthesis-pos">Codex: {row.codexPosition}</span>
          </div>
          {row.resolution ? (
            <div className="synthesis-note">{row.resolution}</div>
          ) : null}
        </>
      );
    case "open":
      return (
        <>
          <div className="synthesis-claim">{row.question}</div>
          {row.raisedBy ? (
            <div className="synthesis-meta">
              <span className="synthesis-sources">{row.raisedBy}</span>
            </div>
          ) : null}
        </>
      );
  }
}
