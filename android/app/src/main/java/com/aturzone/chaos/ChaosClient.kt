package com.aturzone.chaos

import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.HttpURLConnection
import java.net.URL

/**
 * Talks to a `chaos-serve` on the same network.
 *
 * **No HTTP library.** `HttpURLConnection` and `org.json` are both in the
 * Android framework, so this app has no dependencies at all beyond Kotlin's own
 * standard library -- the same rule the Rust side of Chaos keeps. An APK with
 * nothing in it but the app is also an APK that cannot break because something
 * else was upgraded.
 *
 * Every call blocks. Callers run them on a thread; there is no coroutine
 * machinery here because there is nothing for it to do that a thread does not.
 */
class ChaosClient(private val baseUrl: String, private val apiKey: String) {

    class Failed(message: String) : Exception(message)

    /**
     * A message in the conversation. `role` is "user" or "assistant".
     */
    data class Message(val role: String, val content: String)

    private fun open(path: String, method: String): HttpURLConnection {
        val url = URL(baseUrl.trimEnd('/') + path)
        val c = url.openConnection() as HttpURLConnection
        c.requestMethod = method
        // A model can take a while to produce its first token, especially while
        // it is still reading experts off a disk. Ten seconds to *connect* is
        // generous; the read timeout has to be much longer or a slow first
        // token looks exactly like a dead server.
        c.connectTimeout = 10_000
        c.readTimeout = 300_000
        if (apiKey.isNotEmpty()) {
            c.setRequestProperty("Authorization", "Bearer $apiKey")
        }
        c.setRequestProperty("Accept", "application/json")
        return c
    }

    /**
     * What the server says it is serving, or a failure with the reason.
     *
     * This is the connection test: it is the cheapest endpoint that proves the
     * address is right, the network reaches it, and the key is accepted.
     * Distinguishing those three is the whole value -- "could not connect" is
     * not a diagnosis.
     */
    fun models(): List<String> {
        val c = open("/v1/models", "GET")
        try {
            val code = c.responseCode
            if (code == 401 || code == 403) {
                throw Failed("the server rejected the API key")
            }
            if (code != 200) {
                throw Failed("the server answered HTTP $code")
            }
            val body = c.inputStream.bufferedReader().readText()
            val data = JSONObject(body).optJSONArray("data") ?: JSONArray()
            return (0 until data.length()).map { data.getJSONObject(it).optString("id") }
        } finally {
            c.disconnect()
        }
    }

    /**
     * Send the conversation and stream the reply, a piece at a time.
     *
     * `onToken` is called on **this** thread, not the main one -- the caller
     * posts to the UI itself, because a client that touched views would be a
     * client that could only be used from an Activity.
     *
     * The endpoint speaks Server-Sent Events: `data: {json}` lines, blank lines
     * between, and a final `data: [DONE]`. Anything that is not a `data:` line
     * is a comment or a keep-alive and is skipped rather than parsed.
     */
    fun chat(messages: List<Message>, onToken: (String) -> Unit) {
        val payload = JSONObject().apply {
            put("model", "chaos")
            put("stream", true)
            put("messages", JSONArray().apply {
                messages.forEach {
                    put(JSONObject().apply {
                        put("role", it.role)
                        put("content", it.content)
                    })
                }
            })
        }

        val c = open("/v1/chat/completions", "POST")
        c.doOutput = true
        c.setRequestProperty("Content-Type", "application/json")
        try {
            c.outputStream.use { it.write(payload.toString().toByteArray()) }

            val code = c.responseCode
            if (code == 401 || code == 403) throw Failed("the server rejected the API key")
            if (code != 200) {
                // **The error body, not just the code.** The server explains
                // itself in JSON -- "no model loaded", "context exceeded" --
                // and throwing away that sentence to report a number is how a
                // fixable problem becomes a mystery.
                val why = c.errorStream?.bufferedReader()?.readText().orEmpty()
                val message = runCatching {
                    JSONObject(why).getJSONObject("error").getString("message")
                }.getOrNull()
                throw Failed(message ?: "the server answered HTTP $code")
            }

            BufferedReader(InputStreamReader(c.inputStream)).use { reader ->
                while (true) {
                    val line = reader.readLine() ?: break
                    if (!line.startsWith("data:")) continue
                    val body = line.removePrefix("data:").trim()
                    if (body == "[DONE]") break
                    if (body.isEmpty()) continue
                    val piece = runCatching {
                        JSONObject(body)
                            .getJSONArray("choices")
                            .getJSONObject(0)
                            .optJSONObject("delta")
                            ?.optString("content")
                            .orEmpty()
                    }.getOrDefault("")
                    if (piece.isNotEmpty()) onToken(piece)
                }
            }
        } finally {
            c.disconnect()
        }
    }
}
