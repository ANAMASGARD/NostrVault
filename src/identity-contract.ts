import { object, integer } from "./vault-contract";
export type Adapter = "browser" | "remote" | "android";
export type Binding = {
  vaultId: string;
  token: string;
  generation: number;
  account: string | null;
  signerGeneration: number;
  consentRevision: number;
};
export type Grants = {
  readable: boolean;
  relayAuth: string[];
  capture: string[];
  replication: string[];
  attachments: string[];
  background: boolean;
  routingMetadata: boolean;
};
export type Capabilities = {
  publicKey: boolean;
  nip04: boolean | null;
  nip44: boolean | null;
  relayAuth: boolean | null;
};
export type IdentityView = {
  state: string;
  account: string | null;
  adapter: Adapter | null;
  capabilities: Capabilities;
  grants: Grants;
  remember: boolean;
  failure: string | null;
  pairing: string | null;
  package: string | null;
};
export type Effect = {
  id: string;
  binding: Binding;
  adapter: Adapter;
  method: string;
  params: string[];
  package: string | null;
  relays: string[];
  messages: unknown[];
  deadline: number;
};
export type IdentityOutput = {
  requestId: string;
  binding: Binding;
  view: IdentityView;
  effect: Effect | null;
};
export type IdentityAction =
  | { kind: "status" | "cancel" | "disconnect" | "reconnect" }
  | {
      kind: "connect";
      adapter: Adapter;
      remember: boolean;
      pairing: string | null;
      relays: string[];
      package: string | null;
    }
  | { kind: "confirm"; account: string }
  | { kind: "grants"; grants: Grants }
  | { kind: "probe"; method: "nip04_decrypt" | "nip44_decrypt" }
  | { kind: "reply"; id: string; value: unknown }
  | { kind: "failure"; id: string; code: string };
function nullable(v: unknown): v is string | null {
  return v === null || typeof v === "string";
}
function strings(v: unknown): v is string[] {
  return (
    Array.isArray(v) &&
    v.length <= 16 &&
    v.every((s: unknown) => typeof s === "string" && s.length <= 131072)
  );
}
function binding(v: unknown): v is Binding {
  return (
    object(v) &&
    typeof v.vaultId === "string" &&
    typeof v.token === "string" &&
    integer(v.generation) &&
    nullable(v.account) &&
    integer(v.signerGeneration) &&
    integer(v.consentRevision)
  );
}
function adapter(v: unknown): v is Adapter {
  return ["browser", "remote", "android"].includes(String(v));
}
export function parseIdentity(v: unknown): IdentityOutput {
  if (
    !object(v) ||
    typeof v.requestId !== "string" ||
    !binding(v.binding) ||
    !object(v.view)
  )
    throw new Error("malformed");
  const a = v.view;
  if (
    ![
      "disconnected",
      "connected",
      "awaiting_confirmation",
      "waiting_for_approval",
      "needs_authorization",
    ].includes(String(a.state)) ||
    !nullable(a.account) ||
    !(a.adapter === null || adapter(a.adapter)) ||
    !nullable(a.failure) ||
    !nullable(a.package) ||
    !nullable(a.pairing) ||
    typeof a.remember !== "boolean" ||
    !object(a.capabilities) ||
    !object(a.grants)
  )
    throw new Error("malformed");
  const c = a.capabilities,
    g = a.grants;
  if (
    typeof c.publicKey !== "boolean" ||
    ![c.nip04, c.nip44, c.relayAuth].every(
      (x) => x === null || typeof x === "boolean",
    ) ||
    typeof g.readable !== "boolean" ||
    typeof g.background !== "boolean" ||
    typeof g.routingMetadata !== "boolean" ||
    ![g.relayAuth, g.capture, g.replication, g.attachments].every(strings)
  )
    throw new Error("malformed");
  if (v.effect !== null) {
    const e = v.effect;
    if (
      !object(e) ||
      typeof e.id !== "string" ||
      !binding(e.binding) ||
      !adapter(e.adapter) ||
      typeof e.method !== "string" ||
      !strings(e.params) ||
      !strings(e.relays) ||
      !nullable(e.package) ||
      !Array.isArray(e.messages) ||
      e.messages.length > 2 ||
      typeof e.deadline !== "number" ||
      !Number.isSafeInteger(e.deadline)
    )
      throw new Error("malformed");
  }
  // Every field above is checked before narrowing the untrusted host response.
  return v as IdentityOutput;
}
