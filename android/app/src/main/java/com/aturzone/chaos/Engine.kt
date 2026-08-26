package com.aturzone.chaos

/**
 * The Chaos engine, running inside this app.
 *
 * # It is allowed to be absent
 *
 * The native library is built by a cross-compile that needs the NDK, and the
 * APK that CI publishes does not carry it yet. **An app that crashed on launch
 * because an optional component was missing would be a worse app than one that
 * works as a client**, so every call here is guarded and [available] is the
 * question to ask before using any of it.
 *
 * `UnsatisfiedLinkError` is an `Error`, not an `Exception`, so `catch (e:
 * Exception)` would not have caught it — the app would have died in the static
 * initialiser with no message a user could act on.
 */
object Engine {

    /** Whether the native engine loaded. False on any APK built without it. */
    val available: Boolean

    init {
        available = try {
            System.loadLibrary("chaos_android")
            true
        } catch (e: UnsatisfiedLinkError) {
            false
        }
    }

    /** The engine's own version, or null when it is not here. */
    fun versionOrNull(): String? = if (available) {
        try {
            version()
        } catch (e: UnsatisfiedLinkError) {
            // The library loaded but this symbol did not: an .so from a
            // different build. Worth surviving rather than crashing.
            null
        }
    } else {
        null
    }

    /**
     * What this phone is, measured by the same `core/probe` the desktop uses.
     *
     * This is what decides which model a given phone can hold, rather than a
     * hard-coded list of handsets that would be wrong the week it shipped.
     */
    fun describeDeviceOrNull(): String? = if (available) {
        try {
            describeDevice()
        } catch (e: UnsatisfiedLinkError) {
            null
        }
    } else {
        null
    }

    /**
     * Start the real Chaos server inside this app.
     *
     * **This is what makes ALONE and CORE mean anything on a phone.** It is
     * the same server the desktop runs as a child process, so the app's own
     * client talks to it with no second protocol and no second token loop.
     *
     * **This blocks for the life of the app** and must be called from a
     * thread. The library used to spawn its own and crashed in
     * `pthread_create` before the server ran; a thread the JVM made works.
     *
     * @param host `127.0.0.1` for ALONE, `0.0.0.0` for CORE
     * @return why it stopped, or an empty string
     */
    fun start(model: String, host: String, port: Int, key: String): String =
        if (!available) {
            "the engine is not in this build"
        } else {
            try {
                startServer(model, host, port, key)
            } catch (e: UnsatisfiedLinkError) {
                "the engine is not in this build"
            }
        }

    /** The .gguf files in a directory, or an empty list. */
    fun models(dir: String): List<String> =
        if (!available) {
            emptyList()
        } else {
            try {
                listModels(dir).lines().filter { it.isNotBlank() }
            } catch (e: UnsatisfiedLinkError) {
                emptyList()
            }
        }

    private external fun version(): String
    private external fun describeDevice(): String
    private external fun startServer(
        model: String,
        host: String,
        port: Int,
        key: String,
    ): String
    private external fun listModels(dir: String): String
}
