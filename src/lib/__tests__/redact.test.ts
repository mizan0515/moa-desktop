import { describe, expect, it } from "vitest";
import { redact } from "../redact";

describe("redact", () => {
  it("scrubs Bearer tokens", () => {
    const out = redact("Authorization: Bearer abc.def_xyz-123");
    expect(out).not.toContain("abc.def_xyz-123");
    expect(out).toContain("***REDACTED***");
  });

  it("scrubs password= in url-encoded form", () => {
    expect(redact("login?user=alice&password=hunter2&next=/")).toBe(
      "login?user=alice&password=***REDACTED***&next=/",
    );
  });

  it("scrubs token=, api_key=, secret=", () => {
    const out = redact("token=foo api_key=bar secret=baz");
    expect(out).toContain("token=***REDACTED***");
    expect(out).toContain("api_key=***REDACTED***");
    expect(out).toContain("secret=***REDACTED***");
    expect(out).not.toContain("foo");
    expect(out).not.toContain("bar");
    expect(out).not.toContain("baz");
  });

  it("scrubs OpenAI sk- and Anthropic sk-ant- keys", () => {
    expect(redact("key=sk-abcdefghijklmnopqrstuv")).toContain("***REDACTED***");
    expect(redact("ANTHROPIC=sk-ant-abcdefghijklmnopqrstuv")).toContain(
      "***REDACTED***",
    );
  });

  it("scrubs AWS access key ids", () => {
    expect(redact("aws=AKIAIOSFODNN7EXAMPLE used")).toBe(
      "aws=***REDACTED*** used",
    );
  });

  it("scrubs Slack and GitHub tokens", () => {
    expect(redact("xoxb-1234567890-abcdefghij")).toBe("***REDACTED***");
    expect(redact("ghp_abcdefghijklmnopqrstuvwxyz0123456789")).toBe(
      "***REDACTED***",
    );
  });

  it("preserves non-secret diagnostic text", () => {
    const safe = "spawn ENOENT: claude not found in PATH (exit 127)";
    expect(redact(safe)).toBe(safe);
  });

  it("scrubs OAuth multi-parameter headers (oauth_signature, oauth_token)", () => {
    const out = redact(
      'Authorization: OAuth oauth_token="secret-token-abc", oauth_signature="sig-xyz", oauth_nonce="n1"',
    );
    expect(out).not.toContain("secret-token-abc");
    expect(out).not.toContain("sig-xyz");
    expect(out).not.toContain("n1");
    expect(out).toContain("***REDACTED***");
  });

  it("scrubs Digest auth headers", () => {
    const out = redact(
      'WWW-Authenticate: Digest username="alice", response="hash-val", nonce="n2"',
    );
    expect(out).not.toContain("hash-val");
    expect(out).toContain("***REDACTED***");
  });

  it("does NOT mangle benign words containing 'secret'/'token'/'auth' (secretary, tokenizer, authority)", () => {
    const out = redact("secretary=alice tokenizer=fast authority=local");
    expect(out).toBe("secretary=alice tokenizer=fast authority=local");
  });

  it("scrubs Basic auth credentials", () => {
    const out = redact("Authorization: Basic QWxhZGRpbjpvcGVuc2VzYW1l");
    expect(out).not.toContain("QWxhZGRpbjpvcGVuc2VzYW1l");
    expect(out).toContain("***REDACTED***");
  });

  it("scrubs JSON-quoted secrets", () => {
    const out = redact('{"password":"hunter2","user":"alice"}');
    expect(out).not.toContain("hunter2");
    expect(out).toContain("***REDACTED***");
    expect(out).toContain("alice");
  });

  it("scrubs prefixed env-var names (AWS_SECRET_ACCESS_KEY, client_secret)", () => {
    const out = redact(
      "env: AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY client_secret=topsecret refresh_token=rfsh1 access_token=acc1",
    );
    expect(out).not.toContain("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
    expect(out).not.toContain("topsecret");
    expect(out).not.toContain("rfsh1");
    expect(out).not.toContain("acc1");
  });

  it("redacts inside a multiline stderr blob", () => {
    const stderr = [
      "Error: request failed",
      "  at fetch (Bearer sk-abcdefghijklmnopqrstuv)",
      "  env.password=oops123",
    ].join("\n");
    const out = redact(stderr);
    expect(out).not.toContain("sk-abcdefghijklmnopqrstuv");
    expect(out).not.toContain("oops123");
    expect(out).toContain("Error: request failed");
  });
});
