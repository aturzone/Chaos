package com.aturzone.chaos

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The one piece of real logic in this app, and the one that cannot be checked
 * by looking at a screen: a state machine over a stream where the thing it is
 * looking for arrives in fragments.
 */
class ThinkFilterTest {

    /** Feed a whole answer one character at a time — the worst case. */
    private fun charByChar(answer: String): String {
        val f = ThinkFilter()
        val out = StringBuilder()
        for (c in answer) out.append(f.accept(c.toString()))
        out.append(f.flush())
        return out.toString()
    }

    private fun inPieces(vararg pieces: String): String {
        val f = ThinkFilter()
        val out = StringBuilder()
        for (p in pieces) out.append(f.accept(p))
        out.append(f.flush())
        return out.toString()
    }

    @Test
    fun `an answer with no thinking passes through unchanged`() {
        assertEquals("The capital of France is Paris.",
            charByChar("The capital of France is Paris."))
    }

    /** What the emulator actually showed, before this class existed. */
    @Test
    fun `a think block is removed`() {
        assertEquals("\n\nThe capital of France is **Paris**.",
            charByChar("<think>\n\n</think>\n\nThe capital of France is **Paris**."))
    }

    /**
     * **The tags arrive split.** Qwen3 emits `<`, `think`, `>` as three
     * tokens, so a filter that looked at each piece alone would see no tag at
     * all and print every one of them.
     */
    @Test
    fun `a tag split across pieces is still a tag`() {
        assertEquals("Paris.",
            inPieces("<", "think", ">", " working ", "<", "/think", ">", "Paris."))
    }

    /**
     * A tail that could still become a tag is held, and released once it
     * cannot be. Emitting it early leaves stray characters in the transcript
     * for ever; holding it for ever loses text the model actually wrote.
     */
    @Test
    fun `a partial tag is held and then released`() {
        val f = ThinkFilter()
        assertEquals("", f.accept("<thi"))       // could still become <think>
        assertEquals("<thing", f.accept("ng"))   // it could not, so release it
        assertEquals("", f.accept(""))           // and never emit it twice
        assertEquals(" b", f.accept(" b"))
    }

    @Test
    fun `a literal less-than at the very end survives`() {
        assertEquals("a < b", charByChar("a < b"))
        assertEquals("ends with <", charByChar("ends with <"))
    }

    /**
     * **An unterminated block must not eat the answer.** A truncated stream
     * would otherwise show nothing at all, which is indistinguishable from a
     * server that never replied.
     */
    @Test
    fun `an unclosed think block is released rather than swallowed`() {
        val out = charByChar("before <think> cut off")
        assertTrue("got: $out", out.contains("before"))
        assertTrue("got: $out", out.contains("cut off"))
    }

    @Test
    fun `it reports whether it is thinking`() {
        val f = ThinkFilter()
        assertFalse(f.thinking)
        f.accept("<think>")
        assertTrue(f.thinking)
        f.accept("scratch")
        assertTrue(f.thinking)
        f.accept("</think>")
        assertFalse(f.thinking)
        assertTrue(f.thoughtAndFinished)
    }

    @Test
    fun `two blocks in one answer are both removed`() {
        assertEquals("one two three",
            charByChar("one <think>a</think>two <think>b</think>three"))
    }

    /** Chunked arbitrarily, the result must not depend on the chunking. */
    @Test
    fun `the result does not depend on how the stream was cut`() {
        val answer = "start <think>hidden</think> middle <think>x</think> end <"
        val whole = inPieces(answer)
        for (size in 1..7) {
            val pieces = answer.chunked(size).toTypedArray()
            assertEquals("chunk size $size", whole, inPieces(*pieces))
        }
    }
}
