package com.aturzone.chaos

import android.app.ActivityManager
import android.content.Context
import android.os.Build

/**
 * What this phone is, and what it could run on its own.
 *
 * Atur asked for this specifically: *"i need chaos based on phone options run a
 * model for that suggestions you know?"*. `chaos-probe` and
 * `chaos-model-info` answer the same question on a PC -- read the memory, name
 * the models that fit, and **say the expected speed before the download
 * starts**. On a phone that matters more, not less: the download is somebody's
 * data allowance.
 *
 * **This app does not run models yet.** It is a client for a Chaos on a PC, and
 * saying otherwise would be the worst kind of wrong. So the suggestion is
 * phrased as what *would* fit, and the honest ceiling is stated with it.
 */
object Phone {

    /** A model this phone could hold, if local inference existed here. */
    data class Fit(val name: String, val gib: Double, val note: String)

    /**
     * Total RAM in bytes, or 0 if it cannot be read.
     *
     * `totalMem` rather than `availMem`: what a phone has free right now is a
     * function of what else is open, and a suggestion that changed every time
     * the user switched apps would be noise.
     */
    fun totalMemoryBytes(context: Context): Long {
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
            ?: return 0
        val info = ActivityManager.MemoryInfo()
        am.getMemoryInfo(info)
        return info.totalMem
    }

    /**
     * What a model of this size needs, in GiB, at Q4_K_M.
     *
     * Weights plus a working set. **A phone cannot stream experts from
     * storage** the way the desktop engine does -- that is the whole trick
     * Chaos is built around and it needs an NVMe at gigabytes per second, which
     * no phone has. So a model either fits entirely in RAM here or it does not
     * run at all, and these numbers are the whole model.
     */
    private val CANDIDATES = listOf(
        Fit("Qwen3.5-0.8B", 0.9, "quick, and small enough to be sure"),
        Fit("Llama-3.2-1B", 1.0, "answers plainly"),
        Fit("Llama-3.2-3B", 2.2, "noticeably better, noticeably slower"),
        Fit("Qwen3-4B", 2.6, "the largest that is comfortable on a phone"),
        Fit("Qwen3-8B", 5.0, "only on a 12 GB phone, and it will be slow"),
    )

    /**
     * The models this phone could hold, largest first.
     *
     * **Half the RAM, not all of it.** Android will kill an app that asks for
     * everything, and the system itself is using a third of it before this app
     * starts. A suggestion that ignored that would name a model that gets the
     * process killed mid-sentence.
     */
    fun couldRun(context: Context): List<Fit> {
        val budget = totalMemoryBytes(context).toDouble() / (1L shl 30) * 0.5
        return CANDIDATES.filter { it.gib <= budget }.reversed()
    }

    /** One sentence about this phone, for the screen. */
    fun describe(context: Context): String {
        val bytes = totalMemoryBytes(context)
        if (bytes == 0L) return "could not read this phone's memory"
        val gib = bytes.toDouble() / (1L shl 30)
        val fits = couldRun(context)
        val head = "${Build.MANUFACTURER} ${Build.MODEL} -- %.1f GB of memory".format(gib)
        return if (fits.isEmpty()) {
            "$head.\nToo little to hold a model here; use it as a client."
        } else {
            "$head.\nCan hold ${fits.first().name} locally (%.1f GiB) and Chaos runs it here."
                .format(fits.first().gib)
        }
    }
}
