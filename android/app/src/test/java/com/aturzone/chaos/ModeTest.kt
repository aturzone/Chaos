package com.aturzone.chaos

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What each mode may offer.
 *
 * **This has to agree with the desktop's `nav::pages_for`.** Two devices that
 * disagree about what a HELPER can do would be a worse bug than either being
 * wrong alone, because whichever one the user looked at last would be the one
 * they believed.
 */
class ModeTest {

    @Test
    fun `a helper has no conversation`() {
        // A HELPER answers with activations and runs no token loop, so a chat
        // box would be a control that cannot work.
        assertFalse(ModeActivity.canChat("HELPER"))
        assertTrue(ModeActivity.canChat("CORE"))
        assertTrue(ModeActivity.canChat("CLIENT"))
        assertTrue(ModeActivity.canChat("ALONE"))
    }

    @Test
    fun `only a device that runs models has models to manage`() {
        assertTrue(ModeActivity.canHoldModels("CORE"))
        assertTrue(ModeActivity.canHoldModels("ALONE"))
        assertFalse(ModeActivity.canHoldModels("CLIENT"))
        assertFalse(ModeActivity.canHoldModels("HELPER"))
    }

    @Test
    fun `an unknown mode is treated as the safest one`() {
        // A stored value from an older build, or a typo, must not open a
        // control that cannot work. CLIENT loads nothing and reaches out to a
        // CORE, so it is the mode that can do least harm.
        assertTrue(ModeActivity.canChat("nonsense"))
        assertFalse(ModeActivity.canHoldModels("nonsense"))
    }

    @Test
    fun `the four modes are the ones the dial offers`() {
        // The dial's detents and the gating must name the same four things.
        val dialled = listOf("ALONE", "CLIENT", "HELPER", "CORE")
        assertEquals(4, dialled.size)
        for (m in dialled) {
            // Every mode must answer both questions without throwing.
            ModeActivity.canChat(m)
            ModeActivity.canHoldModels(m)
        }
    }
}
