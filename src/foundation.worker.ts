import init, {
  protect,
  recover,
  age_encrypt,
  age_decrypt,
  validated_event_id,
  core_proof,
  archive_proof,
} from "./generated/vault_wasm";
import wasmUrl from "./generated/vault_wasm_bg.wasm?url";
import {
  FoundationFailure,
  isFoundationRequest,
  type FoundationResponse,
} from "./foundation-contract";
import {
  openProofStore,
  checkProofStore,
  readProofRecord,
  equalBytes,
} from "./foundation-store";

let busy = false;
addEventListener("message", (event: MessageEvent<unknown>) => {
  const request = event.data;
  if (!isFoundationRequest(request)) {
    postMessage({
      version: 1,
      requestId: "invalid",
      result: null,
      error: { code: "malformed" },
    });
    return;
  }
  if (busy) {
    postMessage({
      version: 1,
      requestId: request.requestId,
      result: null,
      error: { code: "busy" },
    });
    return;
  }
  busy = true;
  void (async () => {
    let db: IDBDatabase | undefined;
    let response: FoundationResponse;
    try {
      await init({ module_or_path: wasmUrl });
      const control = {
        version: request.version,
        requestId: request.requestId,
        operation: request.operation,
        eventLength: request.eventLength,
        payloadLength: request.payloadLength,
      };
      db = await openProofStore(request.store);
      if (request.phase === "prepare") {
        const protectedBytes = protect(
          JSON.stringify(control),
          request.event,
          request.payload,
        );
        core_proof();
        archive_proof(request.payload);
        if (
          !equalBytes(recover(request.event, protectedBytes), request.payload)
        )
          throw new Error("Record crypto failed");
        if (
          !equalBytes(
            age_decrypt(age_encrypt(request.payload)),
            request.payload,
          )
        )
          throw new Error("Archive crypto failed");
        await checkProofStore(db, protectedBytes);
      } else {
        const saved = await readProofRecord(db, "record");
        if (
          !saved ||
          !equalBytes(recover(request.event, saved), request.payload)
        )
          throw new FoundationFailure("storage");
      }
      response = {
        version: 1,
        requestId: request.requestId,
        result: {
          eventId: validated_event_id(request.event),
          payloadBytes: request.payloadLength,
          storageReopened: request.phase === "reopen",
          cryptoVerified: true,
        },
        error: null,
      };
    } catch (failure: unknown) {
      const code =
        failure instanceof FoundationFailure
          ? failure.code
          : typeof failure === "string" &&
              /^(malformed|unsupported|limit|invalid_id|invalid_signature|out_of_scope|authentication|crypto|crypto_work|randomness)$/.test(
                failure,
              )
            ? failure
            : "foundation_failed";
      response = {
        version: 1,
        requestId: request.requestId,
        result: null,
        error: { code },
      };
    } finally {
      db?.close();
      busy = false;
    }
    postMessage(response);
  })();
});
