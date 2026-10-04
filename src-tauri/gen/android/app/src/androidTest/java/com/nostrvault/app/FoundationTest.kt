package com.nostrvault.app

import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.json.JSONTokener
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class FoundationTest {
    private fun findWebView(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) {
            for (index in 0 until view.childCount) {
                val found = findWebView(view.getChildAt(index))
                if (found != null) return found
            }
        }
        return null
    }

    private fun evaluate(scenario: ActivityScenario<MainActivity>, script: String): String {
        val latch = CountDownLatch(1)
        var result = "null"
        scenario.onActivity { activity ->
            val webView = findWebView(activity.window.decorView)
            checkNotNull(webView) { "Native Activity has no WebView" }
            webView.evaluateJavascript(script) { value ->
                result = value
                latch.countDown()
            }
        }
        assertTrue("WebView evaluation timed out", latch.await(10, TimeUnit.SECONDS))
        return JSONTokener(result).nextValue().toString()
    }

    private fun awaitText(
        scenario: ActivityScenario<MainActivity>,
        script: String,
        expected: String,
    ) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(30)
        var actual = ""
        while (System.nanoTime() < deadline) {
            actual = evaluate(scenario, script)
            if (actual == expected) return
            Thread.sleep(100)
        }
        assertEquals(expected, actual)
    }

    @Test
    fun bundledAppCallsRustAndSurvivesActivityRecreation() {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            val status = "document.querySelector('[role=status]')?.textContent"
            awaitText(scenario, "document.title", "NostrVault")
            awaitText(scenario, status, "Not checked")
            evaluate(scenario, "document.querySelector('button').click()")
            awaitText(scenario, status, "Native runtime ready: android · 0.1.0")
            evaluate(
                scenario,
                "window.__permissionResult = 'pending'; window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {event:'foundation-check',payload:null}).then(() => window.__permissionResult = 'allowed', (error) => window.__permissionResult = String(error))",
            )
            awaitText(
                scenario,
                "window.__permissionResult.includes('event.emit not allowed') && window.__permissionResult.includes('core:event:allow-emit')",
                "true",
            )
            scenario.recreate()
            awaitText(scenario, status, "Not checked")
            evaluate(scenario, "document.querySelector('button').click()")
            awaitText(scenario, status, "Native runtime ready: android · 0.1.0")
            evaluate(scenario, "document.querySelectorAll('button')[1].click()")
            awaitText(
                scenario,
                "document.querySelector('[data-testid=foundation-result]')?.textContent",
                "Rust foundation ready · 152 fixture bytes · encrypted storage reopened",
            )
        }
    }
}
