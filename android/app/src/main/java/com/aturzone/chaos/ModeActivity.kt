package com.aturzone.chaos

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.widget.Button
import android.widget.TextView

/**
 * The launch screen: what is this device?
 *
 * # Why it is the launcher and not a dialog
 *
 * Atur: *"the first mode is selected and the additional options are not all
 * messy."* The choice decides what every other screen means, so it is asked
 * first and once, and the app that follows shows only what the mode can do.
 *
 * # It remembers
 *
 * A device that asked this on every launch would be a device that asks a
 * question it already knows the answer to. The mode is stored, and the dial
 * starts where it was left; [MainActivity] has a CHANGE MODE control that
 * comes back here, which is Atur's *"option to change the mode to exit this
 * mode and enter other modes."*
 */
class ModeActivity : Activity() {

    private lateinit var knob: ModeKnob
    private lateinit var name: TextView
    private lateinit var desc: TextView

    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved)
        setContentView(R.layout.activity_mode)

        knob = findViewById(R.id.knob)
        name = findViewById(R.id.mode_name)
        desc = findViewById(R.id.mode_desc)

        val prefs = getSharedPreferences(PREFS, MODE_PRIVATE)
        knob.setMode(prefs.getString(KEY_MODE, "CLIENT") ?: "CLIENT")
        show(knob.mode())

        // Turning it updates the description under the dial, so the choice is
        // made with its consequence visible rather than from one word.
        knob.onPick = { show(it) }

        findViewById<Button>(R.id.enter).setOnClickListener {
            val picked = knob.mode()
            prefs.edit().putString(KEY_MODE, picked).apply()
            startActivity(
                Intent(this, MainActivity::class.java)
                    .putExtra(EXTRA_MODE, picked),
            )
            finish()
        }
    }

    private fun show(mode: String) {
        name.text = mode
        desc.setText(
            when (mode) {
                "CORE" -> R.string.desc_core
                "HELPER" -> R.string.desc_helper
                "ALONE" -> R.string.desc_alone
                else -> R.string.desc_client
            },
        )
    }

    companion object {
        const val PREFS = "chaos"
        const val KEY_MODE = "mode"
        const val EXTRA_MODE = "mode"

        /**
         * Which parts of the app a mode can use.
         *
         * **The same split as the desktop's `nav::pages_for`.** A HELPER
         * answers with activations and has no token loop, so a chat box would
         * be a control that cannot work. A CLIENT loads nothing here, so it has
         * nothing to manage.
         */
        fun canChat(mode: String): Boolean = mode != "HELPER"

        /** Only a device that runs models locally has anything to manage. */
        fun canHoldModels(mode: String): Boolean = mode == "CORE" || mode == "ALONE"
    }
}
