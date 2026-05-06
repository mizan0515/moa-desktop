// Token Jaccard similarity for claim equivalence detection.
// Order-invariant (semantically identical claims paraphrased differently still match)
// and dependency-free. Threshold tuned empirically against synthesis.md samples
// in __tests__/similarity.test.ts.

const STOPWORDS = new Set([
  "a", "an", "the", "is", "are", "be", "to", "of", "in", "on", "at", "for", "and",
  "or", "but", "with", "by", "from", "as", "that", "this", "it", "its", "was",
  "were", "has", "have", "had", "can", "could", "should", "would", "may", "might",
  "must", "do", "does", "did", "not", "no", "into", "than", "then", "so",
]);

/** Tokenize: lowercase, split on non-alphanumeric, drop stopwords, drop length<2. */
export function tokenize(s: string): string[] {
  return s
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter((t) => t.length >= 2 && !STOPWORDS.has(t));
}

/** Jaccard similarity over token sets. Returns 0 for either-empty inputs. */
export function jaccard(a: string, b: string): number {
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
