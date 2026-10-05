package com.nostrvault.app

import android.view.accessibility.AccessibilityNodeInfo
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Explicit acceptance only. Run with the pinned Amber and a disposable emulator. */
@RunWith(AndroidJUnit4::class)
class RealSignerTest : VaultUiBase() {
    private fun find(node: AccessibilityNodeInfo?, text: String): AccessibilityNodeInfo? {
        if (node == null) return null
        if (
            node.packageName?.toString() == "com.greenart7c3.nostrsigner" &&
                node.text?.toString()?.equals(text, ignoreCase = true) == true
        )
            return node
        for (i in 0 until node.childCount) {
            val found = find(node.getChild(i), text)
            if (found != null) return found
        }
        return null
    }

    private fun approve(text: String) {
        val automation = InstrumentationRegistry.getInstrumentation().uiAutomation
        val info = automation.serviceInfo
        info.flags =
            info.flags or
                android.accessibilityservice.AccessibilityServiceInfo
                    .FLAG_RETRIEVE_INTERACTIVE_WINDOWS
        automation.serviceInfo = info
        val until = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(25)
        while (System.nanoTime() < until) {
            var node =
                automation.windows.firstNotNullOfOrNull { find(it.root, text) }
                    ?: find(automation.rootInActiveWindow, text)
            if (node != null) {
                while (node != null && !node.isClickable) node = node.parent
                if (node?.performAction(AccessibilityNodeInfo.ACTION_CLICK) == true) return
            }
            Thread.sleep(150)
        }
        throw AssertionError("Expected external signer approval control unavailable")
    }

    @Test
    fun pinnedAmberConnectionAndDecrypt() {
        assertTrue(
            "Explicit real-signer gate required",
            InstrumentationRegistry.getArguments().getString("realSigners") == "true",
        )
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            waitFor(scenario, "document.body.textContent.includes('Create vault')")
            fill(scenario, "Password", "M04 disposable password")
            fill(scenario, "Confirm password", "M04 disposable password")
            evaluate(
                scenario,
                "Array.from(document.querySelectorAll('label')).find(l=>l.textContent.includes('I understand')).querySelector('input').click()",
            )
            click(scenario, "Create vault")
            waitFor(
                scenario,
                "Array.from(document.querySelectorAll('button')).some(b=>b.textContent==='Connect' && !b.disabled)",
            )
            click(scenario, "Connect")
            approve("Connect")
            waitFor(scenario, "document.body.textContent.includes('Confirm this is the account')")
            click(scenario, "Confirm account")
            waitFor(scenario, "document.body.textContent.includes('Account connected')")
            evaluate(
                scenario,
                "Array.from(document.querySelectorAll('label')).find(l=>l.textContent.includes('Readable private chats:')).querySelector('input').click()",
            )
            waitFor(
                scenario,
                "document.body.textContent.includes('Approve private-message capability check')",
            )
            click(scenario, "Approve private-message capability check")
            approve("Decrypt")
            waitFor(scenario, "document.body.textContent.includes('Account connected')")
            click(scenario, "Disconnect and remove local signer access")
            waitFor(
                scenario,
                "Array.from(document.querySelectorAll('button')).some(b=>b.textContent==='Connect' && !b.disabled)",
            )
        }
    }
}
