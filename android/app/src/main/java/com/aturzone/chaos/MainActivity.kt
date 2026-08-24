package com.aturzone.chaos

import android.app.Activity
import android.content.Context
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.ScrollView
import android.widget.TextView

/**
 * Chaos on a phone: a client for a Chaos running on a PC.
 *
 * # Which of the two Android apps this is
 *
 * `docs/graph/backlog/android-app.md` sets out the choice that had to be made
 * before a line was written, because "an Android app" is satisfied by both and
 * disappointed by the wrong one:
 *
 * 1. A small-model runner -- real inference on the phone, 1B to 4B, fully
 *    resident. Needs the NDK, ggml cross-compiled for `aarch64-linux-android`,
 *    and the Rust core built for the device.
 * 2. A client for a Chaos on a PC -- no inference here at all.
 *
 * **This is (2), and Atur asked for both, in this order.** The reason is the
 * arithmetic: Chaos exists to run models that do not fit in memory by streaming
 * experts off an NVMe at 2.74 GiB/s. A phone has neither that storage nor that
 * bandwidth, and a 144 GB container is not going onto it at all. A client makes
 * the *big* models usable from the phone, which local inference never can, and
 * it needs no NDK -- so it is both the more useful half and the shorter path.
 *
 * `Phone.kt` answers the other half of his request -- what this device could
 * run on its own -- and says plainly that it is not doing so yet.
 *
 * # No androidx
 *
 * Framework `Activity` and framework views. The Rust side of this project has
 * zero dependencies on principle; there is no reason for the phone half to
 * arrive with a hundred. It also means the APK builds from the Android SDK and
 * the Kotlin plugin alone, which matters when the SDK's own download host is
 * unreachable from where this is developed.
 */
class MainActivity : Activity() {

    private lateinit var address: EditText
    private lateinit var key: EditText
    private lateinit var status: TextView
    private lateinit var phone: TextView
    private lateinit var transcript: TextView
    private lateinit var scroll: ScrollView
    private lateinit var input: EditText
    private lateinit var send: Button
    private lateinit var connect: Button

    private val ui = Handler(Looper.getMainLooper())
    private val history = mutableListOf<ChaosClient.Message>()
    private var busy = false

    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved)
        setContentView(R.layout.activity_main)

        address = findViewById(R.id.address)
        key = findViewById(R.id.key)
        status = findViewById(R.id.status)
        phone = findViewById(R.id.phone)
        transcript = findViewById(R.id.transcript)
        scroll = findViewById(R.id.scroll)
        input = findViewById(R.id.input)
        send = findViewById(R.id.send)
        connect = findViewById(R.id.connect)

        val prefs = getSharedPreferences("chaos", Context.MODE_PRIVATE)
        address.setText(prefs.getString("address", "http://192.168.1.10:8080"))
        key.setText(prefs.getString("key", ""))

        // **The engine's own reading wins when it is here.** `Phone.describe`
        // asks Android; `Engine.describeDevice` runs the same `core/probe` the
        // desktop uses, in this process. When the native library is absent --
        // every APK CI has published so far -- the Android reading is what
        // there is, and the app carries on as a client.
        val engine = Engine.describeDeviceOrNull()
        phone.text = if (engine != null) {
            "engine ${Engine.versionOrNull() ?: "?"} on this phone: $engine"
        } else {
            Phone.describe(this)
        }

        connect.setOnClickListener { testConnection() }
        send.setOnClickListener { sendMessage() }
        send.isEnabled = false
    }

    override fun onPause() {
        super.onPause()
        remember()
    }

    /**
     * Keep the address and key.
     *
     * **Not only in `onPause`.** That was the first version, and it loses them
     * whenever the process is *killed* rather than paused — swiped away from
     * recents, or reclaimed by the system, both of which skip `onPause`
     * entirely. Found by force-stopping the app on an emulator and watching the
     * server address revert to the placeholder.
     *
     * So it is saved on CONNECT as well, which is the moment the values are
     * known to be the ones the user meant.
     */
    private fun remember() {
        getSharedPreferences("chaos", Context.MODE_PRIVATE).edit()
            .putString("address", address.text.toString().trim())
            .putString("key", key.text.toString().trim())
            .apply()
    }

    private fun client(): ChaosClient =
        ChaosClient(address.text.toString().trim(), key.text.toString().trim())

    /**
     * Ask the server what it is serving.
     *
     * **The one thing that separates three different failures**: a wrong
     * address, a firewall, and a rejected key all look identical from a chat
     * box that simply never answers.
     */
    private fun testConnection() {
        val c = client()
        // Saved here, not only on the way out: see `remember`.
        remember()
        setStatus("connecting...")
        connect.isEnabled = false
        Thread {
            val result = runCatching { c.models() }
            ui.post {
                connect.isEnabled = true
                result.onSuccess { models ->
                    if (models.isEmpty()) {
                        setStatus("connected, but no model is loaded -- load one in the Chaos window")
                        send.isEnabled = false
                    } else {
                        setStatus("connected -- ${models.joinToString(", ")}")
                        send.isEnabled = true
                    }
                }.onFailure { e ->
                    // The exception's own message where there is one: "the
                    // server rejected the API key" is worth more than the class
                    // name of whatever threw.
                    setStatus("not connected -- ${e.message ?: e.javaClass.simpleName}")
                    send.isEnabled = false
                }
            }
        }.start()
    }

    private fun sendMessage() {
        if (busy) return
        val text = input.text.toString().trim()
        if (text.isEmpty()) return

        input.setText("")
        history += ChaosClient.Message("user", text)
        append("\n\nyou\n$text\n\nchaos\n")
        busy = true
        send.isEnabled = false
        setStatus("thinking...")

        val c = client()
        val messages = history.toList()
        val reply = StringBuilder()
        // **A reasoning model's scratch work is not the answer.** Run against
        // Qwen3.5 this page showed a bare `<think>` and `</think>` around an
        // empty line before the reply. The tags arrive split across pieces, so
        // they cannot be filtered one piece at a time -- see `ThinkFilter`.
        val think = ThinkFilter()
        Thread {
            val result = runCatching {
                c.chat(messages) { piece ->
                    reply.append(piece)
                    val visible = think.accept(piece)
                    val busy = think.thinking
                    // **Posted, not written directly.** `onToken` runs on this
                    // thread, and touching a view from it is the crash that
                    // does not reproduce on a fast phone.
                    ui.post {
                        if (visible.isNotEmpty()) append(visible)
                        if (busy) setStatus("thinking...")
                    }
                }
            }
            ui.post {
                busy = false
                send.isEnabled = true
                // Anything the filter was still holding back -- a trailing `<`,
                // or an unterminated block, which is released rather than eaten
                // so a truncated stream does not look like an empty answer.
                val rest = think.flush()
                if (rest.isNotEmpty()) append(rest)
                result.onSuccess {
                    // **The scratch work is kept in the history**, though it is
                    // not shown: it is what the model said, and dropping it
                    // would make the next turn's context differ from what the
                    // server has.
                    history += ChaosClient.Message("assistant", reply.toString())
                    setStatus("ready")
                }.onFailure { e ->
                    // The half-finished answer stays on screen: it is what the
                    // model actually said before the connection went, and
                    // deleting it would hide the only evidence of where it got
                    // to. It is not added to the history, because a truncated
                    // turn would poison every turn after it.
                    setStatus("failed -- ${e.message ?: e.javaClass.simpleName}")
                }
            }
        }.start()
    }

    private fun append(text: String) {
        transcript.append(text)
        scroll.post { scroll.fullScroll(View.FOCUS_DOWN) }
    }

    private fun setStatus(text: String) {
        status.text = text
    }
}
