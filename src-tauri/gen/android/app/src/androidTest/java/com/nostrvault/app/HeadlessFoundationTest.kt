package com.nostrvault.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.runner.lifecycle.ActivityLifecycleMonitorRegistry
import androidx.test.runner.lifecycle.Stage
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class HeadlessFoundationTest {
    @Test
    fun rustProofRunsBeforeAnyActivityAndRejectsInvalidRequests() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.runOnMainSync {
            for (stage in Stage.values()) {
                assertTrue(
                    ActivityLifecycleMonitorRegistry.getInstance()
                        .getActivitiesInStage(stage)
                        .isEmpty()
                )
            }
        }
        val context = instrumentation.targetContext.applicationContext
        val request =
            """{"version":1,"requestId":"headless","operation":"foundation_proof","eventLength":373,"payloadLength":152}"""
        val response = JSONObject(HeadlessFoundation.run(context, request))
        assertTrue(response.isNull("error"))
        assertEquals("headless", response.getString("requestId"))
        assertTrue(response.getJSONObject("result").getBoolean("storageReopened"))
        assertTrue(response.getJSONObject("result").getBoolean("cryptoVerified"))
        for ((input, expected) in listOf("{" to "malformed", "x".repeat(1025) to "limit")) {
            val failure = JSONObject(HeadlessFoundation.run(context, input))
            assertTrue(failure.isNull("result"))
            assertEquals(expected, failure.getJSONObject("error").getString("code"))
        }
    }
}
