package com.aturzone.chaos

import android.content.Context
import java.io.File

/**
 * The Chaos engine on this device.
 *
 * # There is no JNI here any more, and that was a decision
 *
 * The engine was loaded into this process and called through JNI for two
 * releases. It worked for anything that did not touch a thread, and broke
 * **bionic's per-thread bookkeeping** the moment anything did — twice, in two
 * different directions:
 *
 * ```text
 * creating a thread from the library
 *   pthread_create -> __init_tcb        SIGSEGV/SEGV_ACCERR
 *
 * a thread that had called the library, exiting
 *   pthread_exit -> pthread_key_clean_all
 *                -> libcrypto thread_local_destructor -> OPENSSL_free
 *                                       SIGSEGV/SEGV_MAPERR
 * ```
 *
 * A 16 MiB stack did not help. Moving the call to a thread the JVM made did
 * not help — the crash moved deeper. The library declares no `PT_TLS` segment,
 * so there is nothing obvious to blame, and removing an unsound
 * `std::env::set_var` fixed a real bug without changing this one.
 *
 * **The same engine, built as an executable, runs on the same device and makes
 * threads perfectly** — `chaos-run` proved that, and `libchaos_serve.so` is
 * doing it now. So the engine is a **child process**, exactly as it is on the
 * desktop, and the three things the bridge used to answer are answered here in
 * Kotlin instead:
 *
 * - the version is a build constant
 * - the device is [Phone], which asks Android
 * - the models are a directory listing
 *
 * None of them needed native code. Keeping a library that corrupts a thread's
 * teardown in order to avoid `File.listFiles()` would be a bad trade.
 */
object Engine {

    /**
     * Whether a local engine binary is present to run.
     *
     * Not "did a library load": the engine is a file that gets executed.
     */
    fun available(context: Context): Boolean = binary(context).canExecute()

    /**
     * The engine binary, shipped in `jniLibs` so Android extracts it and
     * permits executing it from `nativeLibraryDir`.
     */
    fun binary(context: Context): File =
        File(context.applicationInfo.nativeLibraryDir, "libchaos_serve.so")

    /** The version this app was built from, which is the engine's too. */
    fun version(): String = BuildConfig.VERSION_NAME

    /** What this device is, measured rather than guessed. */
    fun describeDevice(context: Context): String = Phone.describe(context)

    /**
     * The `.gguf` files in a directory.
     *
     * Sorted, so the list does not reshuffle between visits — the first entry
     * is what the app loads, and that must not depend on directory order.
     */
    fun models(dir: String): List<String> =
        File(dir).listFiles()
            ?.filter { it.isFile && it.name.endsWith(".gguf", ignoreCase = true) }
            ?.map { it.name }
            ?.sorted()
            ?: emptyList()
}
