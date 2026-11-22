package se.frediver.k7

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ForgeReturnTest {
    private val origin = "http://127.0.0.1:18789"

    @Test
    fun theSignInsReturnIsForwardedToTheFrontsCallbackWithItsQueryAsSent() {
        assertEquals(
            "$origin/api/files/git/forge/callback?code=a%2Bb&state=xyz",
            ForgeReturn.forward("rocks.k7.daw://callback?code=a%2Bb&state=xyz", origin),
        )
        assertEquals(
            "$origin/api/files/git/forge/callback?error=access_denied&state=s",
            ForgeReturn.forward("ROCKS.K7.DAW://callback?error=access_denied&state=s", origin),
        )
    }

    @Test
    fun anythingElseIsNotForwarded() {
        assertNull(ForgeReturn.forward(null, origin))
        assertNull(ForgeReturn.forward("rocks.k7.daw://callback", origin))
        assertNull(ForgeReturn.forward("rocks.k7.daw://other?code=a&state=b", origin))
        assertNull(ForgeReturn.forward("https://callback?code=a&state=b", origin))
        assertNull(ForgeReturn.forward("not a uri at all %%", origin))
    }
}
