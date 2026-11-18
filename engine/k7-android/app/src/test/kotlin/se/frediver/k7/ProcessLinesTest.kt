// ProcessLinesTest.kt — a reader whose stream is closed under it ends quietly, as Android's
// InterruptedIOException does when a stop closes a child's pipe; and finish never leaves one behind.

package se.frediver.k7

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.InputStream
import java.io.InterruptedIOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class ProcessLinesTest {
    /** Serves `first`, then blocks in read until closed, then throws what Android's FileInputStream throws. */
    private class ClosedUnderRead(first: String = "") : InputStream() {
        private val closed = CountDownLatch(1)
        private val head = first.byteInputStream()

        override fun read(): Int {
            val b = head.read()
            if (b >= 0) return b
            closed.await()
            throw InterruptedIOException("read interrupted by close() on another thread")
        }

        override fun read(b: ByteArray, off: Int, len: Int): Int {
            val n = head.read(b, off, len)
            return if (n > 0) n else read()
        }

        override fun close() = closed.countDown()
    }

    private fun uncaught(t: Thread): Array<Throwable?> {
        val seen = arrayOfNulls<Throwable>(1)
        t.uncaughtExceptionHandler = Thread.UncaughtExceptionHandler { _, e -> seen[0] = e }
        return seen
    }

    @Test
    fun aStreamClosedUnderReadEndsTheReaderWithOneLogLine() {
        val stream = ClosedUnderRead()
        val logged = mutableListOf<String>()
        val reader = Thread({ ProcessLines.drain(stream, "out", { logged.add(it) }) {} }, "out")
        val seen = uncaught(reader)
        reader.start()
        Thread.sleep(50)
        stream.close()
        reader.join(2_000)
        assertFalse(reader.isAlive)
        assertNull(seen[0])
        assertEquals(1, logged.size)
        assertTrue(logged[0].startsWith("out ended with its process"))
    }

    @Test
    fun everyLineReachesOnLineUntilTheEnd() {
        val lines = mutableListOf<String>()
        val reader = ProcessLines.pump("a\nb\nc\n".byteInputStream(), "out", {}) { lines.add(it) }
        reader.join(2_000)
        assertEquals(listOf("a", "b", "c"), lines)
    }

    @Test
    fun finishCutsOffAReaderWhosePipeIsStillHeld() {
        val stream = ClosedUnderRead("last words\n")
        val lines = mutableListOf<String>()
        val drained = CountDownLatch(1)
        val reader = ProcessLines.pump(stream, "out", {}) {
            lines.add(it)
            drained.countDown()
        }
        val seen = uncaught(reader)
        assertTrue(drained.await(2, TimeUnit.SECONDS))
        // Nothing more comes and nothing ends the pipe, as when a grandchild holds it: only finish ends the reader.
        ProcessLines.finish(reader, stream, 100)
        reader.join(2_000)
        assertFalse(reader.isAlive)
        assertNull(seen[0])
        assertEquals(listOf("last words"), lines)
    }
}
