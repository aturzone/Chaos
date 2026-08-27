package com.aturzone.chaos

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The one piece of [BrandActivity] that is testable without a device.
 *
 * `normalise` sits between a text box a person types into and a URL that gets
 * concatenated with a path, which is exactly the join where a missing scheme or
 * a pasted trailing slash produces `http://192.168.1.20:8080//qr` or a load of
 * `192.168.1.20:8080/qr` as a relative path off a fictitious origin. Neither
 * fails loudly; both just show nothing.
 */
class BrandTest {

    @Test
    fun a_bare_host_and_port_becomes_an_origin() {
        assertEquals("http://192.168.1.20:8080", BrandActivity.normalise("192.168.1.20:8080"))
    }

    @Test
    fun a_scheme_already_there_is_left_alone() {
        assertEquals("http://10.0.0.2:9000", BrandActivity.normalise("http://10.0.0.2:9000"))
        assertEquals("https://node.local", BrandActivity.normalise("https://node.local"))
    }

    @Test
    fun trailing_slashes_and_whitespace_go() {
        assertEquals("http://10.0.0.2:8080", BrandActivity.normalise("  10.0.0.2:8080/  "))
        assertEquals("http://10.0.0.2:8080", BrandActivity.normalise("http://10.0.0.2:8080///"))
    }

    /**
     * The address box holds an OpenAI base URL, which conventionally ends in
     * `/v1`. The pages are not under it, and `http://host:8080/v1/qr` is a 404
     * that looks exactly like a broken WebView.
     */
    @Test
    fun the_openai_suffix_is_dropped() {
        assertEquals("http://10.0.0.2:8080", BrandActivity.normalise("http://10.0.0.2:8080/v1"))
        assertEquals("http://10.0.0.2:8080", BrandActivity.normalise("10.0.0.2:8080/v1/"))
    }

    @Test
    fun nothing_stays_nothing() {
        assertEquals("", BrandActivity.normalise(""))
        assertEquals("", BrandActivity.normalise("   "))
    }
}
