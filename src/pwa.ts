/** App-shell names only. Vault records stay in IndexedDB, not this list. */
export const SHELL_CACHE = [
  "/",
  "/index.html",
  "/manifest.webmanifest",
] as const;

export function mayCacheAppShell(url: string): boolean {
  return SHELL_CACHE.includes(url as (typeof SHELL_CACHE)[number]);
}
