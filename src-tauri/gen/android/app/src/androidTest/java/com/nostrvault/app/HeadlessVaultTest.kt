package com.nostrvault.app

import android.app.Activity
import android.app.Application
import android.os.Bundle
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class HeadlessVaultTest {
    @Test
    fun runtimeAndKdfNeedNoActivity() {
        val context = ApplicationProvider.getApplicationContext<Application>()
        var activityCreated = false
        val observer =
            object : Application.ActivityLifecycleCallbacks {
                override fun onActivityCreated(activity: Activity, state: Bundle?) {
                    activityCreated = true
                }

                override fun onActivityStarted(activity: Activity) {}

                override fun onActivityResumed(activity: Activity) {}

                override fun onActivityPaused(activity: Activity) {}

                override fun onActivityStopped(activity: Activity) {}

                override fun onActivitySaveInstanceState(activity: Activity, state: Bundle) {}

                override fun onActivityDestroyed(activity: Activity) {}
            }
        context.registerActivityLifecycleCallbacks(observer)
        try {
            val response =
                JSONObject(
                    HeadlessVault.run(
                        context,
                        """{"version":1,"requestId":"headless-status","token":"","generation":0,"vaultId":null,"operation":{"kind":"status"}}""",
                    )
                )
            assertEquals("absent", response.getJSONObject("status").getString("state"))
            val benchmark = JSONObject(HeadlessVault.nativeBenchmark())
            assertEquals(5, benchmark.getJSONArray("milliseconds").length())
            for (index in 0 until 5) assertTrue(
                benchmark.getJSONArray("milliseconds").getDouble(index) > 0
            )
            context.getFileStreamPath("m03-kdf.json").writeText(benchmark.toString())
            context.getFileStreamPath("m03-plaintext-control").writeText("android fixture password")
            assertFalse(activityCreated)
        } finally {
            context.unregisterActivityLifecycleCallbacks(observer)
        }
    }
}
