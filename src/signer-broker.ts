import { object } from "./vault-contract";
import type { Effect } from "./identity-contract";
let outstanding = false;
function provider(): Record<string, unknown> {
  const value: unknown =
    Object.getOwnPropertyDescriptor(window, "nostr")?.value ??
    Reflect.get(window, "nostr");
  if (!object(value)) throw new Error("missing_signer");
  return value;
}
async function call(
  target: Record<string, unknown>,
  name: string,
  args: unknown[],
  signal: AbortSignal,
): Promise<unknown> {
  signal.throwIfAborted();
  const method = target[name];
  if (typeof method !== "function") throw new Error("unsupported");
  try {
    const result: unknown = await Reflect.apply(method, target, args);
    signal.throwIfAborted();
    return result;
  } catch (error: unknown) {
    // Extension error strings may contain payloads; only an explicit rejection code is classified.
    const denied =
      (object(error) && error.code === 4001) ||
      (error instanceof Error && error.message === "nos2x: denied");
    // eslint-disable-next-line preserve-caught-error
    throw new Error(
      signal.aborted ? "cancelled" : denied ? "denied" : "unavailable",
    );
  }
}
export async function browserSigner(
  effect: Effect,
  signal: AbortSignal,
): Promise<unknown> {
  if (outstanding) throw new Error("unavailable");
  outstanding = true;
  try {
    const signer = provider();
    const account = await call(signer, "getPublicKey", [], signal);
    if (typeof account !== "string" || !/^[0-9a-f]{64}$/.test(account))
      throw new Error("malformed");
    if (effect.binding.account !== null && effect.binding.account !== account)
      throw new Error("wrong_account");
    if (effect.method === "get_public_key")
      return {
        account,
        package: null,
        capabilities: {
          publicKey: true,
          nip04:
            object(signer.nip04) && typeof signer.nip04.decrypt === "function",
          nip44:
            object(signer.nip44) && typeof signer.nip44.decrypt === "function",
          relayAuth: typeof signer.signEvent === "function",
        },
      };
    let value: unknown;
    if (
      effect.method === "nip04_decrypt" ||
      effect.method === "nip44_decrypt"
    ) {
      const method =
        effect.method === "nip04_decrypt" ? signer.nip04 : signer.nip44;
      if (!object(method)) throw new Error("unsupported");
      value = await call(method, "decrypt", effect.params, signal);
    } else if (effect.method === "sign_event")
      value = await call(
        signer,
        "signEvent",
        [JSON.parse(effect.params[0]) as unknown],
        signal,
      );
    else throw new Error("unsupported");
    if (
      provider() !== signer ||
      (await call(signer, "getPublicKey", [], signal)) !== account
    )
      throw new Error("wrong_account");
    if (
      typeof value === "string" &&
      new TextEncoder().encode(value).length > 131072
    )
      throw new Error("malformed");
    return value;
  } finally {
    outstanding = false;
  }
}
