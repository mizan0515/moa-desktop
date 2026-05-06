import { useMemo, useState } from "react";
import SynthesisView from "../components/SynthesisView";
import ClaimLedger from "../components/ClaimLedger";
import {
  parseSynthesisNdjson,
  type ClaimLedgerEntry,
  type SynthesisData,
} from "../lib/synthesisTypes";

const MOCK_NDJSON = `{"event":"start","phase":"synthesis","ts":"2026-05-06T10:01:00Z"}
{"event":"row","column":"verified","claim":"REST endpoints SHOULD use plural nouns for collections.","sources":["claude:c1","codex:c1"],"confidence":"high"}
{"event":"row","column":"verified","claim":"Idempotent verbs (GET, PUT, DELETE) MUST be retry-safe.","sources":["claude:c3","codex:c3"],"confidence":"high"}
{"event":"row","column":"codex_only","claim":"GraphQL reduces over-fetching by 30-60% on mobile.","sources":["codex:c4"],"confidence":"med","note":"Claude did not investigate GraphQL angle."}
{"event":"row","column":"claude_only","claim":"POST is not idempotent unless explicitly designed.","sources":["claude:c2"],"confidence":"high","note":"Codex did not raise idempotency."}
{"event":"row","column":"disagreement","topic":"pagination strategy","claude_position":"cursor preferred for >10k rows (perf)","codex_position":"offset acceptable when total-count UI is required","resolution":"context-dependent — both valid"}
{"event":"row","column":"open","question":"Error envelope: RFC 9457 vs custom?","raised_by":"claude:q1"}
{"event":"row","column":"open","question":"Versioning via URL path vs header — which has lower client-migration cost?","raised_by":"codex:q2"}
{"event":"end","status":"ok"}`;

const MOCK_LEDGER: ClaimLedgerEntry[] = [
  {
    claim: "Plural-noun collections is project convention, not RFC-mandated.",
    evidence: "synthesis.md V1; rfc-editor.org/rfc/rfc7231",
    level: "L3",
    confidence: "high",
    residualRisk: "team disagreement on /v1/user vs /v1/users not yet adjudicated",
  },
  {
    claim: "Cursor pagination is the default for >10k row endpoints.",
    evidence: "src/api/list.ts:42; perf bench 2026-04-30",
    level: "L2",
    confidence: "high",
  },
  {
    claim: "GraphQL reduces over-fetching 30-60% on mobile.",
    evidence: "Codex web_search; not Claude-verified",
    level: "L3",
    confidence: "med",
    residualRisk: "applicability to our REST stack unverified",
  },
  {
    claim: "RFC 9457 problem+json is unresolved for error envelope.",
    evidence: "open question O1",
    level: "none",
    confidence: "low",
    residualRisk: "schema choice blocks error-handling spec",
  },
];

export default function SynthViewDemo() {
  const data: SynthesisData = useMemo(() => parseSynthesisNdjson(MOCK_NDJSON), []);
  const [empty, setEmpty] = useState(false);

  const renderData: SynthesisData = empty
    ? { verified: [], codexOnly: [], claudeOnly: [], disagreement: [], open: [] }
    : data;
  const renderLedger: ClaimLedgerEntry[] = empty ? [] : MOCK_LEDGER;

  return (
    <div className="synthview-demo">
      <div className="synthview-demo-toolbar">
        <button type="button" onClick={() => setEmpty((v) => !v)}>
          {empty ? "Load mock data" : "Show empty state"}
        </button>
        <span className="synthview-demo-hint">
          Resize the window below 720px to see the accordion fallback.
        </span>
      </div>
      <SynthesisView data={renderData} />
      <ClaimLedger entries={renderLedger} />
    </div>
  );
}
