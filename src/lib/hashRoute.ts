// Deep-link → tab id mapping. Lets dev/QA jump to a specific tab via the URL
// hash without pulling in a full router. The single-window Tauri shell uses
// the tab registry as the navigation primitive — this is the minimum surface
// that satisfies the FIX-G "/dev/synthview-demo route" ticket. Both
// `#/dev/synthview-demo` (recommended for SPA shells) and the literal
// `/dev/synthview-demo` pathname (for QA convenience) are accepted.
const ROUTES: Record<string, string> = {
  "/dev/synthview-demo": "dev-synthview",
};

function normalize(input: string | undefined | null): string {
  if (!input) return "";
  // strip leading `#`, then leading `/` to leave a canonical "/dev/..." form
  const noHash = input.startsWith("#") ? input.slice(1) : input;
  return noHash.startsWith("/") ? noHash : `/${noHash}`;
}

export function tabIdForLocation(opts: {
  hash?: string;
  pathname?: string;
}): string | null {
  const fromHash = ROUTES[normalize(opts.hash)];
  if (fromHash) return fromHash;
  const fromPath = ROUTES[normalize(opts.pathname)];
  return fromPath ?? null;
}

export function tabIdForHash(hash: string): string | null {
  return tabIdForLocation({ hash });
}
