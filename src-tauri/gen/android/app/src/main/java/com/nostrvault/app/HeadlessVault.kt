package com.nostrvault.app

import android.content.Context
import java.io.File

/** Same production runtime and OS ownership as Tauri. No Activity or WebView. */
object HeadlessVault {
    init {
        System.loadLibrary("vault_native")
    }

    private external fun nativeRun(request: String, directory: String): String

    external fun nativeBenchmark(): String

    fun run(context: Context, request: String): String {
        val directory = File(context.applicationInfo.dataDir, "vault")
        return nativeRun(request, directory.absolutePath)
    }
}
