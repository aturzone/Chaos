package com.aturzone.chaos

import android.app.Activity
import android.content.Context
import android.content.Intent
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


        // **The mode decides what this screen offers.** Same split as the
        // desktop's `nav::pages_for`: a HELPER answers with activations and
        // has no token loop, so a chat box would be a control that cannot
        // work. Atur: "all the items related to that mode are displayed".
        val mode = intent.getStringExtra(ModeActivity.EXTRA_MODE)
            ?: getSharedPreferences(ModeActivity.PREFS, MODE_PRIVATE)
                .getString(ModeActivity.KEY_MODE, "CLIENT")
            ?: "CLIENT"

        // The way back, which Atur asked for: "an option to change the mode to
        // exit this mode and enter other modes."
        findViewById<Button>(R.id.change_mode).setOnClickListener {
            startActivity(Intent(this, ModeActivity::class.java))
            finish()
        }

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

        val engine: String? = null
        // The note must agree with the dial. It used to read "THIS PHONE IS A
        // CLIENT" in every mode, including the ones where the phone was
        // running a model itself.
        findViewById<TextView>(R.id.chaos_note).setText(
            when (mode) {
                "CORE" -> R.string.in_core
                "HELPER" -> R.string.in_helper
                "ALONE" -> R.string.in_alone
                else -> R.string.in_client
            },
        )

        // **A mode that runs models locally starts the engine here.** ALONE
        // serves itself on loopback; CORE serves the network so other devices
        // can reach this phone. Both are the real server -- the same token
        // loop the desktop runs -- so the client below needs no special case.
        if (ModeActivity.canHoldModels(mode)) {
            startLocalEngine(mode)
        }

        if (!ModeActivity.canChat(mode)) {
            for (id in intArrayOf(R.id.scroll, R.id.input, R.id.send)) {
                findViewById<View>(id).visibility = View.GONE
            }
        }

        phone.text = Phone.describe(this)

        // **After every view field is assigned, not before.** This ran first
        // and crashed on HELPER: its opening tab is MONITOR, which reads
        // `address`, and `lateinit property address has not been initialized`
        // killed the activity. ALONE opens on CHAT and never touched it, so
        // the bug showed in exactly one of four modes.
        wireTabs()

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
    /**
     * Run a model on this phone.
     *
     * **Where the models live matters.** `getExternalFilesDir` is a directory
     * a person can actually reach over USB or with a file manager, which a
     * .gguf of several hundred megabytes has to be: nothing is going to be
     * typed in. It is also app-private, so uninstalling takes the models with
     * it rather than leaving gigabytes behind.
     */
    private fun startLocalEngine(mode: String) {
        val dir = getExternalFilesDir(null)?.absolutePath
        if (dir == null) {
            setStatus("no storage available for models")
            return
        }
        val found = Engine.models(dir)
        if (found.isEmpty()) {
            // Said plainly, with the path, because the alternative is a mode
            // that looks broken when it is only empty.
            setStatus("no model on this phone. Put a .gguf in $dir")
            return
        }
        val model = "$dir/${found.first()}"
        val host = if (mode == "CORE") "0.0.0.0" else "127.0.0.1"
        // A CORE is reachable from the network, and chaos-serve refuses that
        // without a key -- rightly. One is made here rather than demanded.
        val useKey = if (mode == "CORE") newKey() else ""
        setStatus("starting ${found.first()}...")
        Thread {
            val why = startEngineProcess(model, host, useKey)
            ui.post {
                if (why.isEmpty()) {
                    // Point our own client at it. The engine takes a moment to
                    // load the weights; the client retries as the user sends.
                    address.setText("http://127.0.0.1:$LOCAL_PORT")
                    key.setText(useKey)
                    setStatus(
                        if (mode == "CORE") {
                            "serving ${found.first()} on port $LOCAL_PORT"
                        } else {
                            "running ${found.first()} on this phone"
                        },
                    )
                } else {
                    setStatus(why)
                }
            }
        }.start()
    }

    /**
     * Run the engine as a child process.
     *
     * **Not through JNI, and the reason is written down.** Loading the engine
     * into this process and calling it worked for anything that did not make a
     * thread; the moment `StreamingRunner::new` called `pthread_create` the
     * app died with SIGSEGV/SEGV_ACCERR inside `__init_tcb`. A bigger stack did
     * not help, moving the call to a JVM thread did not help, and the library
     * has no TLS segment to blame. The same engine **as an executable** runs on
     * the same device and makes threads perfectly -- `chaos-run` was verified
     * doing exactly that.
     *
     * So Android does what the desktop window already does: it spawns the
     * server as a child process and talks to it over the API. One
     * architecture, one protocol, and the part that was fighting is gone.
     *
     * Android permits executing a file from `nativeLibraryDir`, which is why
     * the binary ships as `libchaos_serve.so` and the manifest asks for native
     * libraries to be extracted.
     */
    private fun startEngineProcess(model: String, host: String, key: String): String = try {
        val exe = "${applicationInfo.nativeLibraryDir}/libchaos_serve.so"
        val args = mutableListOf(exe, model, "--host", host, "--port", "$LOCAL_PORT")
        if (key.isNotEmpty()) { args += listOf("--api-key", key) }
        val p = ProcessBuilder(args).redirectErrorStream(true).start()
        engine = p
        // Drain the output, or a full pipe stops the engine rather than merely
        // losing its log -- the same trap the desktop window documents.
        Thread {
            p.inputStream.bufferedReader().forEachLine { line ->
                android.util.Log.i("chaos-serve", line)
            }
        }.start()
        ""
    } catch (e: Exception) {
        "could not start the engine: ${e.message}"
    }

    /** A key for a CORE, unguessable enough and made here rather than asked for. */
    private fun newKey(): String {
        val alphabet = "abcdefghijklmnopqrstuvwxyz0123456789"
        val r = java.security.SecureRandom()
        return (1..26).map { alphabet[r.nextInt(alphabet.length)] }.joinToString("")
    }

    /** Which page is showing. */
    private var tab = "CHAT"

    /**
     * The destinations, gated by mode.
     *
     * **The same split as the desktop's `nav::pages_for`, and the reason is the
     * same.** A HELPER answers with activations and runs no token loop, so a
     * chat box would be a control that cannot work; a CLIENT loads nothing
     * here, so it has no models to manage. Every mode keeps SETTINGS, because
     * that is where the address and key live and a mode with no way to reach
     * them would be a dead end.
     */
    private fun wireTabs() {
        val mode = currentMode()
        val pages = mutableListOf<String>()
        if (ModeActivity.canChat(mode)) pages += "CHAT"
        if (ModeActivity.canHoldModels(mode)) pages += "MODELS"
        pages += "MONITOR"
        pages += "SETTINGS"

        findViewById<TextView>(R.id.mode_badge).text = mode

        val buttons = mapOf(
            "CHAT" to R.id.tab_chat,
            "MODELS" to R.id.tab_models,
            "MONITOR" to R.id.tab_monitor,
            "SETTINGS" to R.id.tab_settings,
        )
        for ((name, id) in buttons) {
            val b = findViewById<Button>(id)
            if (name in pages) {
                b.visibility = View.VISIBLE
                b.setOnClickListener { showTab(name) }
            } else {
                // Gone, not disabled: a control that cannot work should not be
                // on screen looking like it nearly can.
                b.visibility = View.GONE
            }
        }
        showTab(pages.first())
    }

    private fun currentMode(): String =
        intent.getStringExtra(ModeActivity.EXTRA_MODE)
            ?: getSharedPreferences(ModeActivity.PREFS, MODE_PRIVATE)
                .getString(ModeActivity.KEY_MODE, "CLIENT")
            ?: "CLIENT"

    private fun showTab(name: String) {
        tab = name
        val pages = mapOf(
            "CHAT" to R.id.page_chat,
            "MODELS" to R.id.page_models,
            "MONITOR" to R.id.page_monitor,
            "SETTINGS" to R.id.page_settings,
        )
        for ((n, id) in pages) {
            findViewById<View>(id).visibility = if (n == name) View.VISIBLE else View.GONE
        }
        when (name) {
            "MODELS" -> refreshModels()
            "MONITOR" -> refreshMonitor()
        }
    }

    /**
     * What models this device can see.
     *
     * Where they come from depends on the mode, and saying which is the point:
     * a CORE lists what is on the phone, a CLIENT lists what the CORE it is
     * talking to has. Two different questions with the same answer shape.
     */
    private fun refreshModels() {
        val note = findViewById<TextView>(R.id.models_note)
        val list = findViewById<TextView>(R.id.models_list)
        val dir = getExternalFilesDir(null)?.absolutePath
        if (dir == null) {
            note.text = getString(R.string.models_no_storage)
            return
        }
        note.text = getString(R.string.models_here, dir)
        list.text = getString(R.string.models_reading)
        Thread {
            val found = Engine.models(dir)
            ui.post {
                list.text = if (found.isEmpty()) {
                    getString(R.string.models_none)
                } else {
                    found.joinToString(System.lineSeparator()) { "  $it" }
                }
            }
        }.start()
    }

    /**
     * What the machine is doing.
     *
     * Measured by `core/probe` through the engine, which is the same code the
     * desktop's MONITOR page uses -- so the two cannot disagree about the same
     * phone.
     */
    private fun refreshMonitor() {
        val out = findViewById<TextView>(R.id.monitor_text)
        val device = Engine.describeDevice(this)
        val engineVersion = Engine.version()
        val running = engine?.isAlive == true
        val lines = listOf(
            getString(R.string.mon_mode, currentMode()),
            getString(R.string.mon_engine, engineVersion),
            getString(R.string.mon_device, device),
            getString(
                R.string.mon_local,
                if (running) getString(R.string.mon_running, LOCAL_PORT) else getString(R.string.mon_stopped),
            ),
            getString(R.string.mon_endpoint, address.text.toString()),
        )
        out.text = lines.joinToString(System.lineSeparator() + System.lineSeparator())
    }

    /** The engine, while it runs. */
    private var engine: Process? = null

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

    companion object {
        /** The port the in-process engine listens on, matching the desktop. */
        const val LOCAL_PORT = 8231
    }
}
