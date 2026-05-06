import { describe, expect, it } from "vitest";
import {
  DEFAULT_COST_CAP,
  EMPTY_SESSION_TELEMETRY,
  adviceForErrorKind,
  evaluateCap,
  fmtTokens,
  fmtUsd,
  totalTokens,
  totalUsd,
} from "../telemetry";

describe("evaluateCap", () => {
  it("returns ok well under both caps", () => {
    expect(evaluateCap(1, 1)).toBe("ok");
  });
  it("warns at 80% of session cap", () => {
    expect(evaluateCap(8, 0)).toBe("warn");
  });
  it("warns at 80% of daily cap", () => {
    expect(evaluateCap(0, 24)).toBe("warn");
  });
  it("exceeds when session reaches cap", () => {
    expect(evaluateCap(10, 0)).toBe("exceeded");
  });
  it("exceeds when daily reaches cap", () => {
    expect(evaluateCap(0, 30)).toBe("exceeded");
  });
  it("respects custom warn_at", () => {
    expect(evaluateCap(5, 0, { ...DEFAULT_COST_CAP, warn_at: 0.4 })).toBe("warn");
  });
});

describe("totalTokens / totalUsd", () => {
  it("sums every field across both workers", () => {
    const t = {
      claude: { input: 1, output: 2, cache_read: 4, cache_create: 8 },
      codex: { input: 16, output: 32, cache_read: 64, cache_create: 128 },
      claude_usd: 1.5,
      codex_usd: 0,
    };
    expect(totalTokens(t)).toBe(255);
    expect(totalUsd(t)).toBe(1.5);
  });
  it("empty default totals to zero", () => {
    expect(totalTokens(EMPTY_SESSION_TELEMETRY)).toBe(0);
    expect(totalUsd(EMPTY_SESSION_TELEMETRY)).toBe(0);
  });
});

describe("adviceForErrorKind", () => {
  it("maps every kind to a non-empty advice triple", () => {
    const kinds = [
      "cli-missing",
      "auth-expired",
      "quota",
      "network",
      "sandbox-denied",
      "malformed-json",
      "timeout",
      "oom",
      "killed",
      "test-fail",
    ] as const;
    for (const k of kinds) {
      const a = adviceForErrorKind(k);
      expect(a.title).toBeTruthy();
      expect(a.detail).toBeTruthy();
      expect(a.remedy).toBeTruthy();
    }
  });
  it("auth-expired remedy mentions login", () => {
    const a = adviceForErrorKind("auth-expired");
    expect(a.remedy.toLowerCase()).toMatch(/login/);
  });
  it("cli-missing detail mentions PATH", () => {
    const a = adviceForErrorKind("cli-missing");
    expect(a.detail).toMatch(/PATH/);
  });
});

describe("formatters", () => {
  it("fmtUsd thresholds at 1 cent", () => {
    expect(fmtUsd(0)).toBe("< $0.01");
    expect(fmtUsd(0.005)).toBe("< $0.01");
    expect(fmtUsd(1.234)).toBe("$1.23");
  });
  it("fmtTokens uses thousands separators", () => {
    expect(fmtTokens(1234567)).toMatch(/1.234.567|1,234,567/);
  });
});
