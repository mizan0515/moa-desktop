// Allowlist of secret patterns to scrub before rendering arbitrary subprocess
// stderr in the UI. The banner used to print stderr verbatim (FIX-G bug #3),
// which leaked tokens copied into Bearer/Authorization headers, AWS keys,
// password=… query strings, and provider API keys. LogPane goes through the
// same pipeline.
//
// Patterns are intentionally narrow — we don't try to redact email or PII,
// only formats that are unambiguously credentials. False negatives on exotic
// formats are acceptable; false positives that mangle non-secret diagnostic
// text are not.

const REDACTED = "***REDACTED***";

interface Rule {
  re: RegExp;
  replace: string;
}

// Order matters: scheme-specific (Bearer/Basic) rules run before the generic
// key=value rule so the credential token is consumed before the header label
// is matched again.
const RULES: Rule[] = [
  // Authorization scheme + opaque credential: Bearer / Basic / Token.
  // Charclass covers JWT (.), base64 (+/=), and underscore.
  {
    re: /\b(Bearer|Basic|Token)\s+[A-Za-z0-9._\-+/=_]+/gi,
    replace: `$1 ${REDACTED}`,
  },

  // Multi-parameter auth schemes (Digest, OAuth) carry comma-separated
  // key="value" pairs — redact the entire header value to end of line so no
  // parameter (oauth_signature, response, nonce, etc.) leaks. Side effect:
  // anything else on the same line after the header is also redacted, which
  // is the safe default for credential headers.
  {
    re: /\b(Digest|OAuth)\s+[^\r\n]+/gi,
    replace: `$1 ${REDACTED}`,
  },

  // Generic key=value / "key":"value" / key: value pairs for credential-ish
  // names. Uses a non-word lookbehind alternative — matches at start of string
  // or after a non-word char that isn't `_` (so `AWS_SECRET_ACCESS_KEY=…` and
  // `client_secret=…` still match even though `\b` would not). Stops at
  // whitespace, quote, comma, semicolon, or `&` so URL query strings, JSON,
  // and env-style dumps all work.
  {
    re: /(^|[^\w])((?:[a-z0-9]+[_-])*(?:password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key(?:[_-]?id)?|secret[_-]?access[_-]?key|client[_-]?secret|refresh[_-]?token|access[_-]?token|authorization|auth)(?:[_-][a-z0-9_-]*)?)"?\s*[=:]\s*("[^"]*"|'[^']*'|[^\s,;&'"]+)/gi,
    replace: `$1$2=${REDACTED}`,
  },

  // OpenAI keys: sk-... and sk-proj-...; Anthropic: sk-ant-... (>=20 base62).
  { re: /\bsk-(?:ant-|proj-)?[A-Za-z0-9_\-]{20,}/g, replace: REDACTED },

  // AWS access key IDs (AKIA / ASIA prefix, 16 upper alnum).
  { re: /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/g, replace: REDACTED },

  // Slack tokens: xoxb-, xoxp-, xoxa-, xoxr-.
  { re: /\bxox[baprs]-[A-Za-z0-9-]{10,}/g, replace: REDACTED },

  // GitHub PATs: ghp_, gho_, ghu_, ghs_, ghr_ (36+ base62 after prefix).
  { re: /\bgh[pousr]_[A-Za-z0-9]{36,}/g, replace: REDACTED },
];

export function redact(input: string): string {
  let out = input;
  for (const { re, replace } of RULES) {
    out = out.replace(re, replace);
  }
  return out;
}
