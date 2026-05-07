import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import ClaimLedger from "../ClaimLedger";
import type { ClaimLedgerEntry } from "../../lib/synthesisTypes";

const ENTRY: ClaimLedgerEntry = {
  claim: "X is true",
  evidence: "src/x.ts:10",
  level: "L2",
  confidence: "high",
};

describe("ClaimLedger", () => {
  it("renders the empty state without entries", () => {
    const html = renderToStaticMarkup(<ClaimLedger entries={[]} />);
    expect(html).toContain("no claims yet");
  });

  it("renders each claim as an inline row (no nested block divs)", () => {
    const html = renderToStaticMarkup(<ClaimLedger entries={[ENTRY]} />);
    // ticket: rows must be inline — only spans inside the <li>, no <div>.
    const li = html.match(/<li[^>]*class="claim-row"[^>]*>([\s\S]*?)<\/li>/);
    expect(li).not.toBeNull();
    expect(li![1]).not.toContain("<div");
  });

  it("caps visible claims at MAX_CLAIMS (5)", () => {
    const many: ClaimLedgerEntry[] = Array.from({ length: 8 }, (_, i) => ({
      ...ENTRY,
      claim: `c${i}`,
    }));
    const html = renderToStaticMarkup(<ClaimLedger entries={many} />);
    expect(html).toContain("(5/8)");
    expect(html).toContain("c0");
    expect(html).toContain("c4");
    expect(html).not.toContain(">c5<");
  });
});
