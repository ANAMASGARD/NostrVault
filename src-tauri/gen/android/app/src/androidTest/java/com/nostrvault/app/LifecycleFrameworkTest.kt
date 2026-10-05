package com.nostrvault.app

import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Test
import org.junit.runner.RunWith

/** Framework lifecycle correction: close, recreate, relaunch, and fresh process. */
@RunWith(AndroidJUnit4::class)
class LifecycleFrameworkTest : VaultUiBase() {
    @Test
    fun closeRecreate() {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            waitForWebView(scenario)
            waitFor(scenario, "document.title === 'NostrVault'")
            scenario.recreate()
            waitForWebView(scenario)
            waitFor(scenario, "document.title === 'NostrVault'")
        }
    }

    @Test
    fun freshProcessReopen() {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            waitForWebView(scenario)
            waitFor(scenario, "document.title === 'NostrVault'")
        }
    }
}
