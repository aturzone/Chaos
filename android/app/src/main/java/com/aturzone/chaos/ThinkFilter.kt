package com.aturzone.chaos

/**
 * Hides a reasoning model's scratch work from the transcript.
 *
 * # Why this is not a one-line replace
 *
 * A reasoning model wraps its working in `<think>...</think>`. Running the app
 * against Qwen3.5 put this on screen:
 *
 * ```text
 * chaos
 * <think>
 *
 * </think>
 *
 * The capital of France is **Paris**.
 * ```
 *
 * **The tags arrive split across streamed pieces.** Qwen3 emits `<`, `think`
 * and `>` as three separate tokens, so filtering each piece as it arrives sees
 * none of them. `chaos-run` has the same problem and solves it the same way —
 * byte-wise over the accumulated text, never by token id, because the tags are
 * ordinary text in most vocabularies and matching ids works on one model and
 * silently fails on the next.
 *
 * # The part that is easy to get wrong
 *
 * A piece ending in `<thi` might become `<think>` or might be the model
 * literally writing `<thi`. Emitting it immediately is wrong in the first case
 * and holding it forever is wrong in the second, so the tail is **held back
 * only as far as a tag could still be forming** and released as soon as it
 * cannot be.
 */
class ThinkFilter {

    private val all = StringBuilder()
    private var emitted = 0

    /** True while the model is inside a `<think>` block. */
    var thinking: Boolean = false
        private set

    /** Whether a block has been seen and closed. */
    var thoughtAndFinished: Boolean = false
        private set

    /**
     * Feed one streamed piece. Returns the text that should be appended to the
     * transcript now, which is often shorter than the piece and sometimes
     * empty.
     */
    fun accept(piece: String): String {
        all.append(piece)
        val text = all.toString()

        val visible = StringBuilder()
        var i = 0
        var open = -1
        while (i < text.length) {
            val start = text.indexOf(OPEN, i)
            if (start < 0) {
                visible.append(text, i, text.length)
                break
            }
            visible.append(text, i, start)
            val end = text.indexOf(CLOSE, start + OPEN.length)
            if (end < 0) {
                // Still inside: everything from here is scratch work.
                open = start
                break
            }
            i = end + CLOSE.length
        }
        thinking = open >= 0
        thoughtAndFinished = !thinking && text.contains(CLOSE)

        // **Hold back a tail that could still become a tag.** Without this, a
        // piece ending in "<thi" is emitted and the three characters stay in
        // the transcript for ever once the rest arrives.
        var stable = visible.length
        if (!thinking) {
            val hold = longestPartialTagSuffix(visible)
            stable -= hold
        }
        if (stable <= emitted) return ""
        val out = visible.substring(emitted, stable)
        emitted = stable
        return out
    }

    /** How many trailing characters could still be the start of `<think>`. */
    private fun longestPartialTagSuffix(s: CharSequence): Int {
        val max = minOf(OPEN.length - 1, s.length)
        for (n in max downTo 1) {
            if (OPEN.startsWith(s.subSequence(s.length - n, s.length).toString())) {
                return n
            }
        }
        return 0
    }

    /**
     * Everything still held back, for the end of a stream.
     *
     * A completion ending in a literal `<` would otherwise lose it, and an
     * unclosed `<think>` would swallow the whole answer silently — so an
     * unterminated block is released rather than eaten.
     */
    fun flush(): String {
        val text = all.toString()
        val visible = if (thinking) {
            // Unterminated. Better a visible tag than a blank answer: the user
            // can see what happened, which "nothing came back" does not allow.
            text
        } else {
            val sb = StringBuilder()
            var i = 0
            while (i < text.length) {
                val start = text.indexOf(OPEN, i)
                if (start < 0) { sb.append(text, i, text.length); break }
                sb.append(text, i, start)
                val end = text.indexOf(CLOSE, start + OPEN.length)
                if (end < 0) { sb.append(text, start, text.length); break }
                i = end + CLOSE.length
            }
            sb.toString()
        }
        if (visible.length <= emitted) return ""
        val out = visible.substring(emitted)
        emitted = visible.length
        return out
    }

    private companion object {
        const val OPEN = "<think>"
        const val CLOSE = "</think>"
    }
}
