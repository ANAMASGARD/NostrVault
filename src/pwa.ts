/** App-shell names only. Vault records stay in IndexedDB, not this list. */
export const SHELL_CACHE = [
  "/",
  "/index.html",
  "/manifest.webmanifest",
] as const;

export function mayCacheAppShell(url: string): boolean {
  return SHELL_CACHE.some((entry) => url === entry || url.endsWith(entry));
}
