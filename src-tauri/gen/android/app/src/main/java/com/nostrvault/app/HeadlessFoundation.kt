package com.nostrvault.app

import android.content.Context
import java.io.File

/** Foundation-only JNI entry point. Does not initialize an Activity or Tauri runtime. */
object HeadlessFoundation {
    init {
        System.loadLibrary("vault_native")
    }

    private external fun nativeRun(request: String, databasePath: String): String

    fun run(context: Context, request: String): String {
        val directory = File(context.applicationContext.filesDir, "m02-proof")
        check(directory.isDirectory || directory.mkdirs()) { "Cannot open foundation directory" }
        return nativeRun(request, File(directory, "headless.sqlite").absolutePath)
    }
}
