import { afterEach, describe, expect, it, vi } from "vitest";
import { browserSigner } from "./signer-broker";
import type { Effect } from "./identity-contract";
const account =
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
const effect: Effect = {
  id: "a".repeat(32),
  adapter: "browser",
  method: "get_public_key",
  params: [],
  binding: {
    vaultId: "vault",
    token: "session",
    generation: 1,
    account: null,
    signerGeneration: 1,
    consentRevision: 0,
  },
  package: null,
  relays: [],
  messages: [],
  deadline: 200,
};
afterEach(() => vi.unstubAllGlobals());
function install(signer: unknown) {
  vi.stubGlobal("window", { nostr: signer });
}
describe("main-page signer broker", () => {
  it("detects optional methods independently without invoking them", async () => {
    const decrypt = vi.fn(),
      signEvent = vi.fn();
    install({
      getPublicKey: () => Promise.resolve(account),
      nip44: { decrypt },
      signEvent,
    });
    expect(
      await browserSigner(effect, new AbortController().signal),
    ).toMatchObject({
      account,
      capabilities: { nip04: false, nip44: true, relayAuth: true },
    });
    expect(decrypt).not.toHaveBeenCalled();
    expect(signEvent).not.toHaveBeenCalled();
  });
  it("classifies missing, denial and generic signer errors without leaking payloads", async () => {
    install(undefined);
    await expect(
      browserSigner(effect, new AbortController().signal),
    ).rejects.toThrow("missing_signer");
    install({
      getPublicKey: () =>
        Promise.reject(Object.assign(new Error("sensitive"), { code: 4001 })),
    });
    await expect(
      browserSigner(effect, new AbortController().signal),
    ).rejects.toThrow(/^denied$/);
    install({
      getPublicKey: () =>
        Promise.reject(new Error("locked with sensitive data")),
    });
    await expect(
      browserSigner(effect, new AbortController().signal),
    ).rejects.toThrow(/^unavailable$/);
  });
  it("rejects wrong account before invoking decryption", async () => {
    const decrypt = vi.fn();
    install({
      getPublicKey: () => Promise.resolve("f".repeat(64)),
      nip44: { decrypt },
    });
    await expect(
      browserSigner(
        {
          ...effect,
          method: "nip44_decrypt",
          binding: { ...effect.binding, account },
        },
        new AbortController().signal,
      ),
    ).rejects.toThrow("wrong_account");
    expect(decrypt).not.toHaveBeenCalled();
  });
  it("cancels late identity responses before any second prompt and serializes approvals", async () => {
    let finish: (value: string) => void = () => {};
    const decrypt = vi.fn(),
      abort = new AbortController();
    install({
      getPublicKey: () =>
        new Promise<string>((resolve) => {
          finish = resolve;
        }),
      nip44: { decrypt },
    });
    const pending = browserSigner(
      { ...effect, method: "nip44_decrypt" },
      abort.signal,
    );
    await expect(browserSigner(effect, abort.signal)).rejects.toThrow(
      "unavailable",
    );
    abort.abort();
    finish(account);
    await expect(pending).rejects.toThrow("cancelled");
    expect(decrypt).not.toHaveBeenCalled();
  });
  it("detects account changes after decrypt without returning its plaintext", async () => {
    const getPublicKey = vi
      .fn()
      .mockResolvedValueOnce(account)
      .mockResolvedValueOnce("f".repeat(64));
    install({
      getPublicKey,
      nip44: { decrypt: () => Promise.resolve("sensitive") },
    });
    await expect(
      browserSigner(
        {
          ...effect,
          method: "nip44_decrypt",
          binding: { ...effect.binding, account },
        },
        new AbortController().signal,
      ),
    ).rejects.toThrow(/^wrong_account$/);
  });
});
