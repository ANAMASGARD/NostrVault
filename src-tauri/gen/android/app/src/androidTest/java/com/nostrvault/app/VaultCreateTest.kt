package com.nostrvault.app

import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class VaultCreateTest : VaultUiBase() {
    @Test
    fun createLockAndRecreate() {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            waitFor(scenario, "document.body.textContent.includes('Create vault')")
            fill(scenario, "Password", "android fixture password")
            fill(scenario, "Confirm password", "android fixture password")
            evaluate(
                scenario,
                "Array.from(document.querySelectorAll('label')).find(l => l.textContent.includes('I understand')).querySelector('input').click()",
            )
            click(scenario, "Create vault")
            waitFor(scenario, "document.body.textContent.includes('Setup saved securely')")
            click(scenario, "Lock now")
            waitFor(
                scenario,
                "Array.from(document.querySelectorAll('button')).some(b => b.textContent === 'Unlock')",
            )
            scenario.recreate()
            waitFor(
                scenario,
                "Array.from(document.querySelectorAll('button')).some(b => b.textContent === 'Unlock')",
            )
        }
    }
}
