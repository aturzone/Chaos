//! The Anthropic Messages API, so Claude Code can be pointed at a Chaos node.
//!
//! # Why this exists and why it is not the OpenAI surface
//!
//! `claude` reads `ANTHROPIC_BASE_URL`, which is how Atur's own `claude-free`
//! command already works — it points at a gateway and runs `claude` unchanged.
//! Pointing it at `http://127.0.0.1:8080` is the same trick with a different
//! host, and then a local model drives the agent.
//!
//! The catch is the protocol. Chaos serves `/v1/chat/completions` in OpenAI's
//! shape; Claude Code speaks `POST /v1/messages` and nothing else. They differ
//! in more than field names:
//!
//! * **`system` is a list of blocks**, not a message with `role: "system"`.
//! * **Message content is a list of blocks** — `text`, `tool_use`,
//!   `tool_result` — not a string.
//! * **Tools are first-class**, and the answer carries `tool_use` blocks with
//!   their own ids that the client echoes back in `tool_result`.
//! * **`stop_reason` is load-bearing**: a client only executes a tool when the
//!   answer says `"tool_use"`. Returning `"end_turn"` on a turn that called a
//!   tool makes the agent stop with an unused tool call, which looks like the
//!   model refusing to work.
//! * **The stream is a sequence of named events** in a fixed order, not a run
//!   of `data:` chunks.
//!
//! # What was measured before writing any of it
//!
//! `research/claude-code-against-a-chaos-node-2026-09-07.md`. A logging server
//! recorded the real request rather than trusting the documentation, and the
//! numbers decide the design:
//!
//! * A bare `claude -p "hi"` sends **162,229 bytes = 40,255 tokens**, against a
//!   32,768-token context on every model on this machine. `--tools` is the
//!   lever: six tools instead of twenty-eight brings it to **11,706**.
//! * That prompt costs **354 s of prefill** on Qwen3-4B and then runs at
//!   1.09 tok/s. Every turn re-prefills, because `network/serve` has no prefix
//!   cache. **That is the next piece of work, and it is what makes this usable
//!   rather than merely correct.**
//!
//! # Tool calling, which Chaos did not have at all
//!
//! There was no `tools` anywhere in the workspace before this. The model is
//! asked for tool calls in **Qwen's own syntax**, because that is what the
//! models here were trained on:
//!
//! ```text
//! <tool_call>
//! {"name": "Read", "arguments": {"file_path": "a.txt"}}
//! </tool_call>
//! ```
//!
//! and [`split_blocks`] turns that back into `tool_use` blocks. A model that
//! answers in prose instead simply produces one text block, which is the right
//! failure: the agent reports that the model did not call a tool, rather than
//! this layer inventing one.

use chaos_grammar::Json;

/// The marker Qwen-family models wrap a tool call in.
const CALL_OPEN: &str = "<tool_call>";
const CALL_CLOSE: &str = "</tool_call>";

/// One tool, as the client declared it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// The `input_schema` object, as JSON text. Kept as text because it goes
    /// straight into the prompt and re-serialising it would only risk changing
    /// it.
    pub schema: String,
}

/// A parsed `POST /v1/messages` body, flattened to what the engine needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub max_tokens: usize,
    pub stream: bool,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<i64>,
    pub stop_sequences: Vec<String>,
    /// Every system block joined. Anthropic sends a list; the chat templates
    /// here take one string.
    pub system: String,
    /// Turns, with content blocks flattened to text.
    pub messages: Vec<(String, String)>,
    pub tools: Vec<ToolDef>,
}

fn get<'a>(v: &'a Json, key: &str) -> Option<&'a Json> {
    match v {
        Json::Obj(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn as_str(v: &Json) -> Option<&str> {
    match v {
        Json::Str(s) => Some(s),
        _ => None,
    }
}

fn as_num(v: &Json) -> Option<f64> {
    match v {
        Json::Num(n) => Some(*n),
        _ => None,
    }
}

/// Render a `Json` value back to compact text.
///
/// Needed for two things that must survive round-tripping unchanged: a tool's
/// `input_schema`, which goes into the prompt, and a `tool_result`'s content,
/// which may be any JSON the client chose.
pub fn to_text(v: &Json) -> String {
    match v {
        Json::Null => "null".into(),
        Json::Bool(b) => b.to_string(),
        Json::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        Json::Str(s) => quote(s),
        Json::Arr(items) => {
            let inner: Vec<String> = items.iter().map(to_text).collect();
            format!("[{}]", inner.join(","))
        }
        Json::Obj(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}:{}", quote(k), to_text(v)))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}

/// A JSON string literal.
///
/// Control characters are escaped as `\u00XX` rather than passed through: a raw
/// newline inside a string is invalid JSON, and tool results routinely contain
/// them.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Flatten one message's `content` to text.
///
/// **`tool_result` is folded into the turn's text rather than dropped.** The
/// client sends it as a block on a `user` turn referring to a `tool_use_id`
/// from the previous answer, and a model that never sees the result cannot
/// continue — it re-calls the same tool forever, which is the failure this
/// function exists to prevent.
fn flatten_content(content: &Json) -> String {
    match content {
        Json::Str(s) => s.clone(),
        Json::Arr(blocks) => {
            let mut parts: Vec<String> = Vec::new();
            for b in blocks {
                let kind = get(b, "type").and_then(as_str).unwrap_or("");
                match kind {
                    "text" => {
                        if let Some(t) = get(b, "text").and_then(as_str) {
                            parts.push(t.to_string());
                        }
                    }
                    "tool_use" => {
                        // The assistant's own previous call, replayed in the
                        // syntax the model produces, so the conversation it
                        // reads is the one it wrote.
                        let name = get(b, "name").and_then(as_str).unwrap_or("");
                        let input = get(b, "input").map(to_text).unwrap_or_else(|| "{}".into());
                        parts.push(format!(
                            "{CALL_OPEN}\n{{\"name\": {}, \"arguments\": {input}}}\n{CALL_CLOSE}",
                            quote(name)
                        ));
                    }
                    "tool_result" => {
                        let inner = get(b, "content")
                            .map(|c| match c {
                                Json::Str(s) => s.clone(),
                                other => flatten_content(other),
                            })
                            .unwrap_or_default();
                        let failed = matches!(get(b, "is_error"), Some(Json::Bool(true)));
                        parts.push(if failed {
                            format!("<tool_result error=\"true\">\n{inner}\n</tool_result>")
                        } else {
                            format!("<tool_result>\n{inner}\n</tool_result>")
                        });
                    }
                    // `thinking`, `image` and anything added later. Named in the
                    // transcript rather than silently dropped, so a model that
                    // behaves oddly has a visible cause.
                    other if !other.is_empty() => parts.push(format!("[{other} block]")),
                    _ => {}
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

impl Request {
    /// Parse a Messages request.
    ///
    /// A real JSON parse, not the substring extractors the OpenAI surface uses:
    /// the body is 162 KB of nested objects and arrays, and `find("\"role\"")`
    /// cannot tell a message's role from the word "role" inside a tool's
    /// description.
    pub fn parse(body: &str) -> Result<Request, String> {
        let v = Json::parse(body).map_err(|e| format!("body is not JSON: {e}"))?;

        let system = match get(&v, "system") {
            Some(Json::Str(s)) => s.clone(),
            Some(arr @ Json::Arr(_)) => flatten_content(arr),
            _ => String::new(),
        };

        let mut messages = Vec::new();
        if let Some(Json::Arr(turns)) = get(&v, "messages") {
            for t in turns {
                let role = get(t, "role")
                    .and_then(as_str)
                    .unwrap_or("user")
                    .to_string();
                let content = get(t, "content").map(flatten_content).unwrap_or_default();
                messages.push((role, content));
            }
        }
        if messages.is_empty() {
            return Err("`messages` is empty or missing".into());
        }

        let mut tools = Vec::new();
        if let Some(Json::Arr(defs)) = get(&v, "tools") {
            for d in defs {
                // A tool with no name is unusable and naming it here beats a
                // model asked to call "".
                let Some(name) = get(d, "name").and_then(as_str) else {
                    continue;
                };
                tools.push(ToolDef {
                    name: name.to_string(),
                    description: get(d, "description")
                        .and_then(as_str)
                        .unwrap_or("")
                        .to_string(),
                    schema: get(d, "input_schema")
                        .map(to_text)
                        .unwrap_or_else(|| "{}".into()),
                });
            }
        }

        Ok(Request {
            // **Clamped, and the clamp is the point.** Claude Code asks for
            // 64,000 which no model here can produce in a human's lifetime at
            // 1 tok/s. The cap is what stops a single turn running for hours;
            // `stop_reason: "max_tokens"` tells the client honestly.
            max_tokens: get(&v, "max_tokens")
                .and_then(as_num)
                .map(|n| n as usize)
                .unwrap_or(512)
                .clamp(1, 4096),
            stream: matches!(get(&v, "stream"), Some(Json::Bool(true))),
            temperature: get(&v, "temperature").and_then(as_num),
            top_p: get(&v, "top_p").and_then(as_num),
            top_k: get(&v, "top_k").and_then(as_num).map(|n| n as i64),
            stop_sequences: match get(&v, "stop_sequences") {
                Some(Json::Arr(items)) => items
                    .iter()
                    .filter_map(as_str)
                    .map(|s| s.to_string())
                    .collect(),
                _ => Vec::new(),
            },
            system,
            messages,
            tools,
        })
    }

    /// The system text the model sees, with the tools described in it.
    ///
    /// **The tools go in the system turn rather than through the template's own
    /// `tools` variable.** `Tokenizer::apply_chat_template` takes a list of
    /// `Message { role, content }` and nothing else, so there is no `tools`
    /// variable to set without threading one through every chat format. Writing
    /// Qwen's documented block here reaches every model that was trained on it,
    /// and a model that was not still receives a plain, legible description of
    /// what it may call.
    pub fn system_with_tools(&self) -> String {
        if self.tools.is_empty() {
            return self.system.clone();
        }
        let mut s = self.system.clone();
        if !s.is_empty() {
            s.push_str("\n\n");
        }
        s.push_str(
            "# Tools\n\nYou may call one or more of the functions below. \
             To call one, emit exactly this, and nothing else in the same turn:\n\n\
             <tool_call>\n{\"name\": \"<function-name>\", \"arguments\": <args-json>}\n\
             </tool_call>\n\n\
             Call a function only when you need its result. Otherwise answer normally.\n\n\
             <tools>\n",
        );
        for t in &self.tools {
            s.push_str(&format!(
                "{{\"name\": {}, \"description\": {}, \"parameters\": {}}}\n",
                quote(&t.name),
                quote(&t.description),
                t.schema
            ));
        }
        s.push_str("</tools>\n");
        s
    }
}

/// One piece of an answer, in Anthropic's vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        /// The `arguments` object as JSON text, ready to embed.
        input: String,
    },
}

/// Remove a reasoning model's `<think>` working from its answer.
///
/// **Qwen3 emits it by default and it is not the answer.** Left in, Claude Code
/// receives *"Okay, the user wants me to reply with exactly hello. Let me check
/// the instructions"* as the assistant's text, and on a short `max_tokens` the
/// whole budget goes to reasoning with no answer at all — which is exactly what
/// the first live request to this endpoint produced.
///
/// Simpler than the phone's `ThinkFilter`, and it can afford to be: that one
/// works on a live stream where `<think>` arrives split across chunks, while
/// this endpoint buffers the whole answer before splitting it (see
/// `messages_stream`), so the tags are always whole here.
///
/// An unterminated `<think>` — the model ran out of budget mid-thought — drops
/// everything from the tag on, because none of it is an answer.
pub fn strip_thinking(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(open) = rest.find("<think>") {
        out.push_str(&rest[..open]);
        let after = &rest[open + "<think>".len()..];
        match after.find("</think>") {
            Some(close) => rest = &after[close + "</think>".len()..],
            None => return out.trim().to_string(),
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// Split raw model output into content blocks.
///
/// **Malformed calls stay text on purpose.** A model that writes `<tool_call>`
/// and then something that is not JSON has not called a tool, and turning that
/// into a `tool_use` block with empty input would make the agent run a tool the
/// model never asked for. The text survives so a human can see what it wrote.
pub fn split_blocks(raw: &str) -> Vec<Block> {
    let stripped = strip_thinking(raw);
    let mut out = Vec::new();
    let mut rest = stripped.as_str();
    let mut n = 0usize;
    while let Some(start) = rest.find(CALL_OPEN) {
        let before = &rest[..start];
        let after = &rest[start + CALL_OPEN.len()..];
        let Some(end) = after.find(CALL_CLOSE) else {
            break;
        };
        let body = after[..end].trim();
        match Json::parse(body) {
            Ok(v) => {
                let name = get(&v, "name").and_then(as_str).unwrap_or("").to_string();
                if name.is_empty() {
                    break;
                }
                if !before.trim().is_empty() {
                    out.push(Block::Text(before.trim().to_string()));
                }
                // `arguments` is what Qwen emits; `input` is what a model
                // copying Anthropic's own vocabulary emits. Both accepted.
                let input = get(&v, "arguments")
                    .or_else(|| get(&v, "input"))
                    .map(to_text)
                    .unwrap_or_else(|| "{}".into());
                out.push(Block::ToolUse {
                    id: format!("toolu_chaos_{n:04}"),
                    name,
                    input,
                });
                n += 1;
                rest = &after[end + CALL_CLOSE.len()..];
            }
            Err(_) => break,
        }
    }
    if !rest.trim().is_empty() {
        out.push(Block::Text(rest.trim().to_string()));
    }
    if out.is_empty() {
        out.push(Block::Text(String::new()));
    }
    out
}

/// Why the turn ended, in Anthropic's vocabulary.
///
/// **`ToolUse` is not cosmetic.** A client executes a tool only when the answer
/// says so; report `EndTurn` on a turn that called one and the agent stops
/// holding an unused call.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stop {
    EndTurn,
    MaxTokens,
    ToolUse,
    StopSequence,
}

impl Stop {
    pub fn as_str(self) -> &'static str {
        match self {
            Stop::EndTurn => "end_turn",
            Stop::MaxTokens => "max_tokens",
            Stop::ToolUse => "tool_use",
            Stop::StopSequence => "stop_sequence",
        }
    }

    /// The reason to report for these blocks, given why generation stopped.
    ///
    /// A tool call wins over hitting the budget: the client has a call to run
    /// either way, and telling it `max_tokens` would make it stop instead.
    pub fn for_blocks(blocks: &[Block], hit_budget: bool) -> Stop {
        if blocks.iter().any(|b| matches!(b, Block::ToolUse { .. })) {
            Stop::ToolUse
        } else if hit_budget {
            Stop::MaxTokens
        } else {
            Stop::EndTurn
        }
    }
}

/// One content block as JSON text.
fn block_json(b: &Block) -> String {
    match b {
        Block::Text(t) => format!("{{\"type\":\"text\",\"text\":{}}}", quote(t)),
        Block::ToolUse { id, name, input } => format!(
            "{{\"type\":\"tool_use\",\"id\":{},\"name\":{},\"input\":{input}}}",
            quote(id),
            quote(name)
        ),
    }
}

/// A whole non-streaming `/v1/messages` response.
pub fn message_json(
    model: &str,
    blocks: &[Block],
    input_tokens: usize,
    output_tokens: usize,
    stop: Stop,
) -> String {
    let content: Vec<String> = blocks.iter().map(block_json).collect();
    format!(
        "{{\"id\":\"msg_chaos_0001\",\"type\":\"message\",\"role\":\"assistant\",\
         \"model\":{},\"content\":[{}],\"stop_reason\":\"{}\",\"stop_sequence\":null,\
         \"usage\":{{\"input_tokens\":{input_tokens},\"output_tokens\":{output_tokens}}}}}",
        quote(model),
        content.join(","),
        stop.as_str()
    )
}

/// One SSE frame: the event name and its payload, framed as the wire wants.
pub fn frame(event: &str, data: &str) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

/// The whole event sequence for an answer, in the order a client requires.
///
/// **Order is the specification here**, so it lives in one function rather than
/// being spread through the handler: `message_start`, then per block a
/// `content_block_start`, its deltas and a `content_block_stop`, then
/// `message_delta` carrying the stop reason and the output count, then
/// `message_stop`.
///
/// Text is delivered as one delta per block rather than per token. The engine
/// generates at about 1 tok/s at the context Claude Code sends, so a client
/// showing words as they arrive gains little, and buffering is what makes a
/// tool call detectable at all — `<tool_call>` cannot be recognised until it
/// has been seen, and text already sent cannot be taken back.
pub fn sse_sequence(
    model: &str,
    blocks: &[Block],
    input_tokens: usize,
    output_tokens: usize,
    stop: Stop,
) -> Vec<String> {
    let mut out = Vec::new();
    out.push(frame(
        "message_start",
        &format!(
            "{{\"type\":\"message_start\",\"message\":{{\"id\":\"msg_chaos_0001\",\
             \"type\":\"message\",\"role\":\"assistant\",\"model\":{},\"content\":[],\
             \"stop_reason\":null,\"stop_sequence\":null,\
             \"usage\":{{\"input_tokens\":{input_tokens},\"output_tokens\":0}}}}}}",
            quote(model)
        ),
    ));
    for (i, b) in blocks.iter().enumerate() {
        match b {
            Block::Text(t) => {
                out.push(frame(
                    "content_block_start",
                    &format!(
                        "{{\"type\":\"content_block_start\",\"index\":{i},\
                         \"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}"
                    ),
                ));
                out.push(frame(
                    "content_block_delta",
                    &format!(
                        "{{\"type\":\"content_block_delta\",\"index\":{i},\
                         \"delta\":{{\"type\":\"text_delta\",\"text\":{}}}}}",
                        quote(t)
                    ),
                ));
            }
            Block::ToolUse { id, name, input } => {
                out.push(frame(
                    "content_block_start",
                    &format!(
                        "{{\"type\":\"content_block_start\",\"index\":{i},\
                         \"content_block\":{{\"type\":\"tool_use\",\"id\":{},\"name\":{},\
                         \"input\":{{}}}}}}",
                        quote(id),
                        quote(name)
                    ),
                ));
                // **The arguments arrive as a string, not as an object.** A
                // `tool_use` block opens with an empty `input` and is filled by
                // `input_json_delta` frames whose `partial_json` concatenate to
                // the JSON text. Sending the object inline in the start frame
                // is the mistake that leaves a client with `{}` for arguments.
                out.push(frame(
                    "content_block_delta",
                    &format!(
                        "{{\"type\":\"content_block_delta\",\"index\":{i},\
                         \"delta\":{{\"type\":\"input_json_delta\",\"partial_json\":{}}}}}",
                        quote(input)
                    ),
                ));
            }
        }
        out.push(frame(
            "content_block_stop",
            &format!("{{\"type\":\"content_block_stop\",\"index\":{i}}}"),
        ));
    }
    out.push(frame(
        "message_delta",
        &format!(
            "{{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"{}\",\
             \"stop_sequence\":null}},\"usage\":{{\"output_tokens\":{output_tokens}}}}}",
            stop.as_str()
        ),
    ));
    out.push(frame("message_stop", "{\"type\":\"message_stop\"}"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_list_and_block_content_flatten() {
        let body = r#"{
            "model":"claude-opus-5","max_tokens":100,"stream":true,
            "system":[{"type":"text","text":"one"},{"type":"text","text":"two"}],
            "messages":[{"role":"user","content":[{"type":"text","text":"hi"}]}]
        }"#;
        let r = Request::parse(body).expect("parses");
        assert_eq!(r.system, "one\ntwo");
        assert_eq!(r.messages, vec![("user".into(), "hi".into())]);
        assert!(r.stream);
        assert_eq!(r.max_tokens, 100);
    }

    /// **The clamp that stops a turn running for hours.** Claude Code asks for
    /// 64,000 tokens; at the ~1 tok/s this engine manages at that context, that
    /// is most of a day.
    #[test]
    fn max_tokens_is_clamped() {
        let body = r#"{"max_tokens":64000,"messages":[{"role":"user","content":"x"}]}"#;
        assert_eq!(Request::parse(body).unwrap().max_tokens, 4096);
    }

    /// A real parse, not substring search: the word "role" appears inside a
    /// tool description here, and an extractor keyed on `"role"` would read it
    /// as a turn.
    #[test]
    fn a_tool_description_mentioning_role_is_not_a_message() {
        let body = r#"{
            "messages":[{"role":"user","content":"go"}],
            "tools":[{"name":"Read","description":"the \"role\" of a file",
                      "input_schema":{"type":"object","properties":{"p":{"type":"string"}}}}]
        }"#;
        let r = Request::parse(body).expect("parses");
        assert_eq!(r.messages.len(), 1);
        assert_eq!(r.tools.len(), 1);
        assert_eq!(r.tools[0].name, "Read");
        assert!(r.tools[0].schema.contains("properties"));
    }

    /// **A tool result the model never sees makes it call the same tool
    /// forever.** The client sends it as a block on a user turn.
    #[test]
    fn a_tool_result_reaches_the_model() {
        let body = r#"{"messages":[
            {"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Read",
                                            "input":{"file_path":"a.txt"}}]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"t1",
                                       "content":"hello from the file"}]}
        ]}"#;
        let r = Request::parse(body).expect("parses");
        assert!(r.messages[0].1.contains("<tool_call>"));
        assert!(r.messages[0].1.contains("\"file_path\":\"a.txt\""));
        assert!(r.messages[1].1.contains("hello from the file"));
        assert!(r.messages[1].1.contains("<tool_result>"));
    }

    #[test]
    fn an_error_result_says_so() {
        let body = r#"{"messages":[{"role":"user","content":[
            {"type":"tool_result","tool_use_id":"t1","content":"boom","is_error":true}]}]}"#;
        let r = Request::parse(body).expect("parses");
        assert!(r.messages[0].1.contains("error=\"true\""));
    }

    #[test]
    fn tools_are_described_in_the_system_turn() {
        let body = r#"{"messages":[{"role":"user","content":"x"}],"system":"be brief",
            "tools":[{"name":"Bash","description":"run it","input_schema":{"type":"object"}}]}"#;
        let s = Request::parse(body).unwrap().system_with_tools();
        assert!(s.starts_with("be brief"));
        assert!(s.contains("<tool_call>"));
        assert!(s.contains("\"name\": \"Bash\""));
        assert!(s.contains("</tools>"));
    }

    #[test]
    fn no_tools_leaves_the_system_turn_alone() {
        let body = r#"{"messages":[{"role":"user","content":"x"}],"system":"be brief"}"#;
        assert_eq!(
            Request::parse(body).unwrap().system_with_tools(),
            "be brief"
        );
    }

    #[test]
    fn a_call_becomes_a_tool_use_block() {
        let raw = "I will look.\n<tool_call>\n{\"name\": \"Read\", \
                   \"arguments\": {\"file_path\": \"a.txt\"}}\n</tool_call>";
        let blocks = split_blocks(raw);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], Block::Text("I will look.".into()));
        match &blocks[1] {
            Block::ToolUse { name, input, id } => {
                assert_eq!(name, "Read");
                assert_eq!(input, "{\"file_path\":\"a.txt\"}");
                assert!(id.starts_with("toolu_"));
            }
            other => panic!("not a tool call: {other:?}"),
        }
    }

    #[test]
    fn two_calls_get_distinct_ids() {
        let raw = "<tool_call>\n{\"name\":\"A\",\"arguments\":{}}\n</tool_call>\
                   <tool_call>\n{\"name\":\"B\",\"arguments\":{}}\n</tool_call>";
        let blocks = split_blocks(raw);
        let ids: Vec<String> = blocks
            .iter()
            .filter_map(|b| match b {
                Block::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1], "a client keys results by id");
    }

    /// **Malformed stays text.** Inventing a `tool_use` block here would make
    /// the agent run something the model never asked for.
    #[test]
    fn a_broken_call_is_left_as_text() {
        for raw in [
            "<tool_call>\nnot json\n</tool_call>",
            "<tool_call>\n{\"arguments\":{}}\n</tool_call>",
            "<tool_call>\n{\"name\":\"A\"",
        ] {
            let blocks = split_blocks(raw);
            assert!(
                blocks.iter().all(|b| matches!(b, Block::Text(_))),
                "{raw:?} produced a tool call"
            );
        }
    }

    #[test]
    fn plain_prose_is_one_text_block() {
        let blocks = split_blocks("just an answer");
        assert_eq!(blocks, vec![Block::Text("just an answer".into())]);
    }

    /// **The reason a client acts on.** A turn that called a tool must say
    /// `tool_use`, even if it also hit the budget.
    #[test]
    fn the_stop_reason_follows_the_blocks() {
        let call = vec![Block::ToolUse {
            id: "t".into(),
            name: "A".into(),
            input: "{}".into(),
        }];
        assert_eq!(Stop::for_blocks(&call, false), Stop::ToolUse);
        assert_eq!(Stop::for_blocks(&call, true), Stop::ToolUse);
        let text = vec![Block::Text("hi".into())];
        assert_eq!(Stop::for_blocks(&text, false), Stop::EndTurn);
        assert_eq!(Stop::for_blocks(&text, true), Stop::MaxTokens);
    }

    #[test]
    fn the_stream_events_are_in_the_required_order() {
        let blocks = vec![
            Block::Text("hi".into()),
            Block::ToolUse {
                id: "t1".into(),
                name: "Read".into(),
                input: "{\"p\":1}".into(),
            },
        ];
        let names: Vec<String> = sse_sequence("m", &blocks, 10, 3, Stop::ToolUse)
            .iter()
            .map(|f| {
                f.lines()
                    .next()
                    .unwrap()
                    .trim_start_matches("event: ")
                    .to_string()
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "message_start",
                "content_block_start",
                "content_block_delta",
                "content_block_stop",
                "content_block_start",
                "content_block_delta",
                "content_block_stop",
                "message_delta",
                "message_stop",
            ]
        );
    }

    /// A `tool_use` block's arguments travel as `partial_json` text. Sending
    /// the object inline in the start frame leaves the client with `{}`.
    #[test]
    fn tool_arguments_travel_as_partial_json() {
        let blocks = vec![Block::ToolUse {
            id: "t1".into(),
            name: "Read".into(),
            input: "{\"p\":1}".into(),
        }];
        let all = sse_sequence("m", &blocks, 1, 1, Stop::ToolUse).join("");
        assert!(all.contains("\"input\":{}"), "start frame must open empty");
        assert!(all.contains("input_json_delta"));
        assert!(all.contains("\"partial_json\":\"{\\\"p\\\":1}\""));
    }

    #[test]
    fn every_frame_is_wire_framed() {
        for f in sse_sequence("m", &[Block::Text("x".into())], 1, 1, Stop::EndTurn) {
            assert!(f.starts_with("event: "));
            assert!(f.contains("\ndata: {"));
            assert!(f.ends_with("\n\n"), "a frame ends on a blank line");
        }
    }

    #[test]
    fn a_whole_message_is_valid_json_with_the_fields_a_client_reads() {
        let blocks = vec![Block::Text("hello".into())];
        let s = message_json("qwen", &blocks, 7, 2, Stop::EndTurn);
        let v = Json::parse(&s).expect("valid JSON");
        assert_eq!(get(&v, "type").and_then(as_str), Some("message"));
        assert_eq!(get(&v, "role").and_then(as_str), Some("assistant"));
        assert_eq!(get(&v, "stop_reason").and_then(as_str), Some("end_turn"));
        let usage = get(&v, "usage").expect("usage");
        assert_eq!(get(usage, "input_tokens").and_then(as_num), Some(7.0));
    }

    /// Newlines are routine in tool results and a raw one is invalid JSON.
    #[test]
    fn control_characters_survive_quoting() {
        let s = quote("a\nb\tc\"d\\e\u{1}");
        assert_eq!(s, "\"a\\nb\\tc\\\"d\\\\e\\u0001\"");
        assert!(Json::parse(&format!("{{\"k\":{s}}}")).is_ok());
    }

    #[test]
    fn an_empty_messages_array_is_refused_by_name() {
        let e = Request::parse(r#"{"messages":[]}"#).expect_err("must refuse");
        assert!(e.contains("messages"), "{e}");
    }

    /// **Reasoning is not the answer**, and the first live request to this
    /// endpoint proved it: Qwen3 spent all 40 tokens of budget inside
    /// `<think>` and Claude Code would have been handed the working as the
    /// assistant's reply.
    #[test]
    fn thinking_is_stripped() {
        assert_eq!(strip_thinking("<think>\nworking\n</think>\nhello"), "hello");
        assert_eq!(strip_thinking("before<think>x</think>after"), "beforeafter");
        assert_eq!(strip_thinking("no tags here"), "no tags here");
        // Two blocks, which a long answer can produce.
        assert_eq!(strip_thinking("<think>a</think>x<think>b</think>y"), "xy");
    }

    /// Out of budget mid-thought: everything from the tag on is working, not
    /// answer, so none of it is kept.
    #[test]
    fn an_unterminated_think_block_drops_the_rest() {
        assert_eq!(strip_thinking("answer<think>and then I ran out"), "answer");
        assert_eq!(strip_thinking("<think>only working"), "");
    }

    #[test]
    fn a_tool_call_after_thinking_is_still_found() {
        let raw = "<think>I should read it</think>\n<tool_call>\n\
                   {\"name\":\"Read\",\"arguments\":{\"p\":1}}\n</tool_call>";
        let blocks = split_blocks(raw);
        assert_eq!(blocks.len(), 1, "the thinking must not survive as text");
        assert!(matches!(&blocks[0], Block::ToolUse { name, .. } if name == "Read"));
    }

    #[test]
    fn a_body_that_is_not_json_is_refused_by_name() {
        let e = Request::parse("not json").expect_err("must refuse");
        assert!(e.contains("not JSON"), "{e}");
    }
}
