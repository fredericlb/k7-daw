// ChildKillTest.kt — the parent pid read from a /proc stat line, whatever the program's name holds.

package se.frediver.k7

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ChildKillTest {
    @Test
    fun theParentIsTheFieldAfterTheState() {
        assertEquals(1541, ChildKill.parentOf("4750 (libk7node.so) T 1541 4750 1970 0 -1 4194560"))
    }

    @Test
    fun aNameWithSpacesAndParenthesesIsSkippedWhole() {
        assertEquals(12, ChildKill.parentOf("99 (a ) b) c) S 12 99 99 0"))
    }

    @Test
    fun aLineWithoutANameHasNoParent() {
        assertNull(ChildKill.parentOf("garbage"))
    }
}
