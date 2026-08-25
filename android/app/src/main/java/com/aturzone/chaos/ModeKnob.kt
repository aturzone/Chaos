package com.aturzone.chaos

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RadialGradient
import android.graphics.RectF
import android.graphics.Shader
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.sin

/**
 * The mode knob: a gas-stove control that chooses what this device is.
 *
 * # Why this is drawn and not an image
 *
 * `assets/knob.svg` is the specification, and the desktop draws it per pixel
 * because plain GDI has no gradient fill. Android's `Canvas` has
 * [RadialGradient] as a primitive, so here the same geometry is drawn with the
 * platform's own 2-D API — which is what `docs/graph/backlog/the-mode-knob.md`
 * decided, and why the SVG is a spec rather than an asset.
 *
 * The radii below are the SVG's own, as fractions of the body radius, so the
 * two platforms cannot drift apart without somebody editing a number.
 *
 * # The badge is not drawn
 *
 * The mark in the middle is `knob_badge.png` under each `drawable` density,
 * rendered from
 * `assets/logo.svg` by `tools/make-android-icons.py` at five densities.
 * **Atur's mark is never redrawn, approximated or regenerated.**
 *
 * # A 180 degree sweep, with stops
 *
 * Four detents across the top, 60 degrees apart. A control that spins forever
 * has no first position and no last; a stove knob travels an arc and stops.
 */
class ModeKnob @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs) {

    /** Degrees from twelve o'clock for each mode, left to right. */
    private val detents = listOf(
        -90f to "ALONE",
        -30f to "CLIENT",
        30f to "HELPER",
        90f to "CORE",
    )

    var angle: Float = -90f
        private set

    /** Called when the dial settles on a mode. */
    var onPick: ((String) -> Unit)? = null

    private val fgColour = resources.getColor(R.color.fg, null)
    private val dimColour = resources.getColor(R.color.fg_tertiary, null)

    /// The detent the dial was last on, so a click sounds when it changes
    /// rather than on every pixel of a drag.
    private var lastDetent: String? = null

    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val text = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        textAlign = Paint.Align.CENTER
    }
    // **Not `as? BitmapDrawable`.** That cast fails silently, leaving this
    // null, and a badge that never draws looks exactly like a badge drawn
    // white on white. A screenshot of the running app said the centre was
    // (255,255,255) with no mark in it. Drawn as a Drawable, which works
    // whatever kind the resource resolves to.
    private val badge = resources.getDrawable(R.drawable.knob_badge, null)

    /** The mode the pointer is nearest, which is what a lift snaps to. */
    fun mode(): String = detents.minByOrNull { kotlin.math.abs(angle - it.first) }!!.second

    fun setMode(name: String) {
        angle = detents.firstOrNull { it.second == name }?.first ?: -90f
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        val w = width.toFloat()
        val h = height.toFloat()
        // Room above for the labels, so the body is not centred in the view.
        val r = min(w, h) * 0.36f
        val cx = w / 2f
        val cy = h * 0.56f

        drawLabels(canvas, cx, cy, r)

        // The skirt, lit from the upper left like everything else.
        paint.shader = RadialGradient(
            cx - r * 0.30f, cy - r * 0.38f, r * 1.55f,
            intArrayOf(0xFFEDEDEA.toInt(), 0xFFDCDCD7.toInt(), 0xFFB6B6B0.toInt(), 0xFF96968F.toInt()),
            floatArrayOf(0f, 0.55f, 0.85f, 1f),
            Shader.TileMode.CLAMP,
        )
        canvas.drawCircle(cx, cy, r, paint)
        paint.shader = null

        drawKnurl(canvas, cx, cy, r)

        // The chamfer, then the face. This ring is most of what makes it read
        // as three-dimensional, which is why it is a gradient and not a stroke.
        paint.shader = RadialGradient(
            cx - r * 0.35f, cy - r * 0.45f, r * 1.30f,
            intArrayOf(0xFFFFFFFF.toInt(), 0xFFCDCDC7.toInt(), 0xFFA4A49E.toInt()),
            floatArrayOf(0f, 0.62f, 1f),
            Shader.TileMode.CLAMP,
        )
        canvas.drawCircle(cx, cy, r * 0.835f, paint)
        paint.shader = RadialGradient(
            cx - r * 0.32f, cy - r * 0.44f, r * 1.48f,
            intArrayOf(
                0xFFFFFFFF.toInt(), 0xFFF8F8F6.toInt(), 0xFFE7E7E3.toInt(),
                0xFFD2D2CD.toInt(), 0xFFBCBCB7.toInt(),
            ),
            floatArrayOf(0f, 0.40f, 0.70f, 0.89f, 1f),
            Shader.TileMode.CLAMP,
        )
        canvas.drawCircle(cx, cy, r * 0.777f, paint)
        paint.shader = null

        drawPointer(canvas, cx, cy, r)

        // The collar, then the badge.
        paint.color = 0xFFD8D8D3.toInt()
        canvas.drawCircle(cx, cy, r * 0.466f, paint)
        paint.color = Color.WHITE
        canvas.drawCircle(cx, cy, r * 0.427f, paint)
        badge?.let {
            val d = (r * 0.427f * 1.86f).toInt()
            it.setBounds(
                (cx - d / 2f).toInt(), (cy - d / 2f).toInt(),
                (cx + d / 2f).toInt(), (cy + d / 2f).toInt(),
            )
            it.draw(canvas)
        }

        // The outer edge, so the knob sits on the page rather than floating.
        paint.style = Paint.Style.STROKE
        paint.strokeWidth = r * 0.014f
        paint.color = 0x22000000
        canvas.drawCircle(cx, cy, r, paint)
        paint.style = Paint.Style.FILL
    }

    /** 48 ridges on the skirt, drawn as short radial strokes. */
    private fun drawKnurl(canvas: Canvas, cx: Float, cy: Float, r: Float) {
        paint.style = Paint.Style.STROKE
        paint.strokeWidth = r * 0.030f
        paint.strokeCap = Paint.Cap.ROUND
        for (i in 0 until 48) {
            val a = (i * 360f / 48f - 90f) * Math.PI.toFloat() / 180f
            // Bright on one side of the ring, dark on the other, so the grip
            // reads as moulded rather than printed.
            val lit = (cos(a + 2.4f) + 1f) / 2f
            val v = (150 + 90 * lit).toInt().coerceIn(0, 255)
            paint.color = Color.argb(150, v, v, v - 4)
            canvas.drawLine(
                cx + r * 0.865f * cos(a), cy + r * 0.865f * sin(a),
                cx + r * 0.960f * cos(a), cy + r * 0.960f * sin(a),
                paint,
            )
        }
        paint.style = Paint.Style.FILL
    }

    /** The indicator, a recess rather than a printed line. */
    private fun drawPointer(canvas: Canvas, cx: Float, cy: Float, r: Float) {
        canvas.save()
        canvas.rotate(angle, cx, cy)
        val half = r * 0.038f
        val top = cy - r * 0.786f
        val bot = cy - r * 0.505f
        paint.color = 0xFF57574F.toInt()
        canvas.drawRoundRect(
            RectF(cx - half, top, cx + half, bot), half, half, paint,
        )
        paint.color = 0x8CFFFFFF.toInt()
        canvas.drawRoundRect(
            RectF(cx + half * 0.15f, top + half, cx + half * 0.75f, bot - half),
            half * 0.3f, half * 0.3f, paint,
        )
        canvas.restore()
    }

    private fun drawLabels(canvas: Canvas, cx: Float, cy: Float, r: Float) {
        val here = mode()
        val baseText = r * 0.150f
        text.textSize = baseText
        for ((deg, name) in detents) {
            val a = (deg - 90f) * Math.PI.toFloat() / 180f
            // **Clamped inside the view.** ALONE sits at the left stop, and at
            // a label radius of 1.30r its centre landed 9px from the view's
            // edge -- so it was drawn and clipped, and a screenshot showed 0
            // lit pixels where the other three showed ~140.
            val halfWord = text.measureText(name) / 2f + 3f
            val lx = (cx + r * 1.22f * cos(a)).coerceIn(halfWord, width - halfWord)
            val ly = cy + r * 1.22f * sin(a) + text.textSize / 3f
            val on = name == here
            // **Theme colours, not hard-coded black.** These were 0xFF111111
            // on a #0D1117 background, which is near-black on near-black: the
            // chosen mode's name was the one you could not read.
            text.color = if (on) fgColour else dimColour
            text.isFakeBoldText = on
            text.textSize = if (on) baseText * 1.12f else baseText
            canvas.drawText(name, lx, ly, text)
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val w = width.toFloat()
        val h = height.toFloat()
        val r = min(w, h) * 0.36f
        val cx = w / 2f
        val cy = h * 0.56f
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                // Generous: the labels are part of the control.
                if (hypot(event.x - cx, event.y - cy) > r * 1.5f) return false
                parent?.requestDisallowInterceptTouchEvent(true)
                aim(event.x - cx, event.y - cy)
            }
            MotionEvent.ACTION_MOVE -> aim(event.x - cx, event.y - cy)
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                // **Snap.** A control that can rest between positions is one
                // that can be left meaning nothing.
                val name = mode()
                setMode(name)
                onPick?.invoke(name)
            }
            else -> return false
        }
        return true
    }

    /// A stove knob clicks as it passes each position, and that click is how
    /// you know it moved without looking at it. `FX_KEY_CLICK` is the system's
    /// own, so it follows whatever the user has set and ships no asset.
    private fun detentFeedback() {
        val now = mode()
        if (now == lastDetent) return
        lastDetent = now
        playSoundEffect(android.view.SoundEffectConstants.CLICK)
        performHapticFeedback(
            android.view.HapticFeedbackConstants.CLOCK_TICK,
            android.view.HapticFeedbackConstants.FLAG_IGNORE_GLOBAL_SETTING,
        )
    }

    private fun aim(dx: Float, dy: Float) {
        // Screen y grows downward; twelve o'clock is zero.
        val deg = Math.toDegrees(atan2(dx.toDouble(), -dy.toDouble())).toFloat()
        // The stops are real: a stove knob does not go round the back, and
        // letting it would put ALONE next to CORE.
        angle = deg.coerceIn(-90f, 90f)
        detentFeedback()
        invalidate()
    }
}
