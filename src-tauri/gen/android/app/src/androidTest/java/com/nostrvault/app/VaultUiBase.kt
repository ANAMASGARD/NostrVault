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
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
open class VaultUiBase {
    protected fun findWebView(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) {
            for (index in 0 until view.childCount) {
                val found = findWebView(view.getChildAt(index))
                if (found != null) return found
            }
        }
        return null
    }

    protected fun evaluate(scenario: ActivityScenario<MainActivity>, script: String): String {
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

    protected fun awaitText(
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

    protected fun waitFor(scenario: ActivityScenario<MainActivity>, expression: String) {
        awaitText(scenario, expression, "true")
    }

    protected fun fill(scenario: ActivityScenario<MainActivity>, label: String, value: String) {
        evaluate(
            scenario,
            "Array.from(document.querySelectorAll('label')).find(l => l.firstChild.textContent === '$label').querySelector('input') && (() => { const input=Array.from(document.querySelectorAll('label')).find(l => l.firstChild.textContent === '$label').querySelector('input'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,'$value'); input.dispatchEvent(new Event('input',{bubbles:true})); })()",
        )
    }

    protected fun click(scenario: ActivityScenario<MainActivity>, text: String) {
        evaluate(
            scenario,
            "Array.from(document.querySelectorAll('button')).find(b => b.textContent === '$text').click()",
        )
    }
}
