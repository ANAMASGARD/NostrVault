package com.nostrvault.app

import android.app.Activity
import android.content.Intent
import androidx.activity.result.ActivityResult
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SignerResultTest {
    private fun args() =
        JSONObject()
            .put("id", "test-request")
            .put("method", "get_public_key")
            .put("package", "selected.signer")
            .put("binding", JSONObject().put("account", "confirmed"))

    private fun reply() =
        Intent().putExtra("id", "test-request").putExtra("package", "selected.signer")

    @Test
    fun distinguishesRejectionCancellationAndMalformedResults() {
        assertEquals(
            "denied",
            SignerResult.failure(
                args(),
                ActivityResult(Activity.RESULT_OK, reply().putExtra("rejected", true)),
            ),
        )
        assertEquals(
            "cancelled",
            SignerResult.failure(
                args(),
                ActivityResult(Activity.RESULT_CANCELED, reply().putExtra("rejected", true)),
            ),
        )
        assertEquals(
            "malformed",
            SignerResult.failure(args(), ActivityResult(Activity.RESULT_OK, null)),
        )
        assertEquals(
            "malformed",
            SignerResult.failure(
                args(),
                ActivityResult(Activity.RESULT_OK, Intent().putExtra("id", "test-request")),
            ),
        )
    }

    @Test
    fun matchesRequestPackageAndAccount() {
        assertNull(SignerResult.failure(args(), ActivityResult(Activity.RESULT_OK, reply())))
        assertEquals(
            "malformed",
            SignerResult.failure(
                args(),
                ActivityResult(Activity.RESULT_OK, reply().putExtra("id", "stale")),
            ),
        )
        assertEquals(
            "wrong_account",
            SignerResult.failure(
                args(),
                ActivityResult(Activity.RESULT_OK, reply().putExtra("package", "other.signer")),
            ),
        )
        assertEquals(
            "wrong_account",
            SignerResult.failure(
                args(),
                ActivityResult(Activity.RESULT_OK, reply().putExtra("current_user", "other")),
            ),
        )
    }
}
