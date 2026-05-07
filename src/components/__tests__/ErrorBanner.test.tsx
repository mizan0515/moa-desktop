import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import ErrorBanner from "../ErrorBanner";

describe("ErrorBanner", () => {
  it("renders nothing without a kind", () => {
    expect(renderToStaticMarkup(<ErrorBanner kind={null} />)).toBe("");
  });

  it("redacts Bearer/sk-/AWS/password secrets in stderr detail", () => {
    const stderr = [
      "spawn failed",
      "  Authorization: Bearer abc.def_xyz-12345",
      "  OPENAI_KEY=sk-abcdefghijklmnopqrstuvwxyz",
      "  password=hunter2",
      "  aws=AKIAIOSFODNN7EXAMPLE",
    ].join("\n");
    const html = renderToStaticMarkup(
      <ErrorBanner kind="cli-missing" detail={stderr} />,
    );
    expect(html).not.toContain("abc.def_xyz-12345");
    expect(html).not.toContain("sk-abcdefghijklmnopqrstuvwxyz");
    expect(html).not.toContain("hunter2");
    expect(html).not.toContain("AKIAIOSFODNN7EXAMPLE");
    expect(html).toContain("***REDACTED***");
    // non-secret diagnostic content survives
    expect(html).toContain("spawn failed");
  });
});
