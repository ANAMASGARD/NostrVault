package com.nostrvault.app

import android.app.Activity
import android.content.Intent
import androidx.activity.result.ActivityResult
import androidx.core.net.toUri
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@TauriPlugin
class SignerPlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingId: String? = null

    @Command
    fun packages(invoke: Invoke) {
        val query = Intent(Intent.ACTION_VIEW, "nostrsigner:".toUri())
        invoke.resolveObject(
            activity.packageManager
                .queryIntentActivities(query, 0)
                .map { it.activityInfo.packageName }
                .distinct()
                .sorted()
        )
    }

    @Command
    fun request(invoke: Invoke) {
        if (pendingId != null) {
            invoke.resolve(JSObject().put("failure", "unavailable"))
            return
        }
        val args = invoke.getArgs()
        val id = args.optString("id")
        val method = args.optString("method")
        val selected = args.optString("package")
        if (
            id.length != 32 ||
                selected.isEmpty() ||
                method !in listOf("get_public_key", "nip04_decrypt", "nip44_decrypt", "sign_event")
        ) {
            invoke.resolve(JSObject().put("failure", "malformed"))
            return
        }
        val params = args.optJSONArray("params")
        val payload =
            when (method) {
                "get_public_key" -> ""
                "sign_event" -> params?.optString(0) ?: ""
                else -> params?.optString(1) ?: ""
            }
        if (payload.length > 131072) {
            invoke.resolve(JSObject().put("failure", "malformed"))
            return
        }
        val intent =
            Intent(Intent.ACTION_VIEW, "nostrsigner:$payload".toUri())
                .setPackage(selected)
                .putExtra("type", method)
                .putExtra("id", id)
        if (method != "get_public_key") {
            intent.putExtra("current_user", args.getJSONObject("binding").getString("account"))
            if (method != "sign_event") intent.putExtra("pubkey", params?.optString(0))
        }
        if (intent.resolveActivity(activity.packageManager) == null) {
            invoke.resolve(JSObject().put("failure", "missing_signer"))
            return
        }
        pendingId = id
        try {
            startActivityForResult(invoke, intent, "signerResult")
        } catch (_: RuntimeException) {
            pendingId = null
            invoke.resolve(JSObject().put("failure", "unavailable"))
        }
    }

    @ActivityCallback
    fun signerResult(invoke: Invoke, result: ActivityResult) {
        val args = invoke.getArgs()
        val id = args.optString("id")
        if (pendingId != id) {
            invoke.resolve(JSObject().put("failure", "cancelled"))
            return
        }
        pendingId = null
        val data = result.data
        val failure = SignerResult.failure(args, result)
        if (failure != null) {
            invoke.resolve(JSObject().put("failure", failure))
            return
        }
        val value = data?.getStringExtra("result")
        if (value == null || value.length > 131072) {
            invoke.resolve(JSObject().put("failure", "malformed"))
            return
        }
        if (args.optString("method") == "get_public_key") {
            val capabilities =
                JSObject()
                    .put("publicKey", true)
                    .put("nip04", org.json.JSONObject.NULL)
                    .put("nip44", org.json.JSONObject.NULL)
                    .put("relayAuth", org.json.JSONObject.NULL)
            invoke.resolve(
                JSObject()
                    .put("account", value)
                    .put("package", data.getStringExtra("package"))
                    .put("capabilities", capabilities)
            )
        } else {
            invoke.resolveObject(value)
        }
    }
}

/** Pure foreground-result validation; session generations are checked again in Rust. */
internal object SignerResult {
    fun failure(args: org.json.JSONObject, result: ActivityResult): String? {
        val data = result.data
        return when {
            result.resultCode != Activity.RESULT_OK -> "cancelled"
            data == null -> "malformed"
            data.getBooleanExtra("rejected", false) -> "denied"
            data.getStringExtra("id") != args.optString("id") -> "malformed"
            args.optString("method") == "get_public_key" && !data.hasExtra("package") -> "malformed"
            data.hasExtra("package") &&
                data.getStringExtra("package") != args.optString("package") -> "wrong_account"
            data.hasExtra("current_user") &&
                data.getStringExtra("current_user") !=
                    args.optJSONObject("binding")?.optString("account") -> "wrong_account"
            else -> null
        }
    }
}
