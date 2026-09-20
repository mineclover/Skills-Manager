/**
 * Folder holding a binary, derived from its full path.
 *
 * Both separators are handled because the path comes from the Rust side, which
 * reports native paths (`/usr/local/bin/skm`, `C:\Users\me\...\skm.exe`).
 * Returns "" when there is no directory component, so callers can fall back to
 * showing the raw path.
 */
export function binaryDir(binaryPath: string): string {
  const lastSep = Math.max(binaryPath.lastIndexOf("/"), binaryPath.lastIndexOf("\\"));
  if (lastSep < 0) return "";
  if (lastSep === 0) return "/";
  return binaryPath.slice(0, lastSep);
}
