package com.nostrvault.app

import android.os.FileObserver
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Minimal native close reproduction, without vault creation or signer requests. */
@RunWith(AndroidJUnit4::class)
class LifecycleDiagnosisTest : VaultUiBase() {
    @Test
    fun launchAndClose() {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            waitForWebView(scenario)
            waitFor(scenario, "document.title === 'NostrVault'")
            if (InstrumentationRegistry.getArguments().getString("nativeDebugger") == "true") {
                val context = InstrumentationRegistry.getInstrumentation().targetContext
                val closed = CountDownLatch(1)
                val observer =
                    object : FileObserver(context.filesDir, CREATE) {
                        override fun onEvent(event: Int, path: String?) {
                            if (path == "native-close") closed.countDown()
                        }
                    }
                observer.startWatching()
                try {
                    assertTrue(
                        "Native debugger close signal timed out",
                        closed.await(60, TimeUnit.SECONDS),
                    )
                } finally {
                    observer.stopWatching()
                }
            }
        }
    }
}
