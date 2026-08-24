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

    private external fun version(): String
    private external fun describeDevice(): String
}
