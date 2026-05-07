// Token Jaccard similarity for claim equivalence detection.
// Order-invariant (semantically identical claims paraphrased differently still match)
// and dependency-free. Threshold tuned empirically against synthesis.md samples
// in __tests__/synthesis.test.ts.
//
// FIX-E: negation-aware. "X is safe" and "X is not safe" must NOT match — that
// previously caused negative claims to land in the verified column. "not"/"no"
// were removed from stopwords so they affect token overlap, and an explicit
// polarity check forces similarity to 0 when the two claims disagree on negation.

const STOPWORDS = new Set([
  "a", "an", "the", "is", "are", "be", "to", "of", "in", "on", "at", "for", "and",
  "or", "but", "with", "by", "from", "as", "that", "this", "it", "its", "was",
  "were", "has", "have", "had", "can", "could", "should", "would", "may", "might",
  "must", "do", "does", "did", "into", "than", "then", "so",
]);

const NEG_EN_RE = /\b(not|no|never|none|neither|nor|cannot|cant|wont|shouldnt|wouldnt|isnt|arent|wasnt|werent|doesnt|didnt|dont|hasnt|havent|hadnt)\b/i;
const NEG_KO_RE = /(없|아니|못해|못한|못함|지\s*않|지않)/;
// Token-level set used by tokenize() to strip the negation cue from the bag
// (polarity is captured separately, so "should not expose" and "should never
// expose" must reduce to the same token bag once polarity matches).
const NEG_TOKEN_SET = new Set([
  "not", "no", "never", "none", "neither", "nor", "cannot", "cant", "wont",
  "shouldnt", "wouldnt", "isnt", "arent", "wasnt", "werent", "doesnt", "didnt",
  "dont", "hasnt", "havent", "hadnt",
]);

function hasNegation(s: string): boolean {
  const flat = s.replace(/['']/g, "");
  return NEG_EN_RE.test(flat) || NEG_KO_RE.test(s);
}

/**
 * Tokenize: lowercase, strip apostrophes, split on non-alphanumeric, drop
 * stopwords, drop negation cues (handled by polarity guard), drop length<2.
 */
export function tokenize(s: string): string[] {
  return s
    .toLowerCase()
    .replace(/['']/g, "")
    .split(/[^\p{L}\p{N}]+/u)
    .filter((t) => t.length >= 2 && !STOPWORDS.has(t) && !NEG_TOKEN_SET.has(t));
}

/** Jaccard similarity over token sets. Returns 0 for either-empty inputs. */
export function jaccard(a: string, b: string): number {
  if (hasNegation(a) !== hasNegation(b)) return 0;
  const ta = new Set(tokenize(a));
  const tb = new Set(tokenize(b));
  if (ta.size === 0 || tb.size === 0) return 0;
  let inter = 0;
  for (const t of ta) if (tb.has(t)) inter++;
  const union = ta.size + tb.size - inter;
  return union === 0 ? 0 : inter / union;
}

/** Default threshold for "same claim" classification. */
export const SIMILARITY_THRESHOLD = 0.85;
