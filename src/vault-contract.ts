export type VaultState =
  | "absent"
  | "creating"
  | "locked"
  | "unlocking"
  | "unlocked"
  | "locking"
  | "changing_password"
  | "migrating";
export type Setup = { version: 1; protected: true; lossAcknowledged: true };
export type VaultStatus = {
  state: VaultState;
  vaultId: string | null;
  token: string;
  generation: number;
  revision: number;
  setup: Setup | null;
};
export type VaultOperation =
  | { kind: "status" | "lock" }
  | { kind: "create"; password: string; confirmation: string }
  | { kind: "unlock"; password: string }
  | { kind: "save_setup"; setup: Setup; revision: number }
  | {
      kind: "change_password";
      current: string;
      password: string;
      confirmation: string;
    };
export type VaultRequest = {
  version: 1;
  requestId: string;
  token: string;
  generation: number;
  vaultId: string | null;
  operation: VaultOperation;
};
export function object(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
export function integer(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= 0 &&
    value <= 0xffffffff
  );
}
export function parseStatus(value: unknown): VaultStatus {
  if (
    !object(value) ||
    typeof value.state !== "string" ||
    ![
      "absent",
      "creating",
      "locked",
      "unlocking",
      "unlocked",
      "locking",
      "changing_password",
      "migrating",
    ].includes(value.state) ||
    (value.vaultId !== null &&
      (typeof value.vaultId !== "string" ||
        !/^[0-9a-f]{32}$/.test(value.vaultId))) ||
    typeof value.token !== "string" ||
    !/^[0-9a-f]{32}$/.test(value.token) ||
    !integer(value.generation) ||
    !integer(value.revision)
  )
    throw new Error("malformed");
  let setup: Setup | null = null;
  if (value.setup !== null) {
    if (
      !object(value.setup) ||
      value.setup.version !== 1 ||
      value.setup.protected !== true ||
      value.setup.lossAcknowledged !== true ||
      value.state !== "unlocked"
    )
      throw new Error("malformed");
    setup = { version: 1, protected: true, lossAcknowledged: true };
  }
  // Validate the discriminant before narrowing the transport representation.
  return {
    state: value.state as VaultState,
    vaultId: value.vaultId,
    token: value.token,
    generation: value.generation,
    revision: value.revision,
    setup,
  };
}
export function validUnicode(value: string): boolean {
  for (let i = 0; i < value.length; i++) {
    const code = value.charCodeAt(i);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(++i);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
    } else if (code >= 0xdc00 && code <= 0xdfff) return false;
  }
  return true;
}
export function validatePasswords(operation: VaultOperation): void {
  for (const key of ["password", "confirmation", "current"] as const) {
    if (key in operation) {
      const value: unknown = Object.getOwnPropertyDescriptor(
        operation,
        key,
      )?.value;
      if (
        typeof value !== "string" ||
        !validUnicode(value) ||
        new TextEncoder().encode(value).length > 1024
      )
        throw new Error("password_policy");
    }
  }
}
export const messages: Record<string, string> = {
  authentication:
    "The password or protected vault data could not be authenticated. Check your password and try again.",
  password_policy:
    "Use at least 12 characters, no more than 1,024 UTF-8 bytes, and matching confirmation.",
  busy: "This vault is in use in another window or process. Lock or close it there, then retry.",
  unsupported:
    "This vault version or required storage capability is not supported.",
  quota:
    "Storage is full or unavailable. Existing committed data has been preserved.",
  conflict: "The saved state changed. Lock and unlock to reload it.",
  cancelled:
    "Operation cancelled. Any completed encrypted write will be checked when you unlock.",
  exists: "A vault already exists. Unlock it to continue.",
  locked: "Unlock your vault to continue.",
};
export function failureCode(error: unknown): string {
  if (typeof error === "string" && /^[a-z_]+$/.test(error)) return error;
  if (error instanceof Error && /^[a-z_]+$/.test(error.message))
    return error.message;
  return "storage";
}
