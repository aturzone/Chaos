//! The description frame Ideogram 4 was conditioned on, for a person to fill in.
//!
//! # Why this looked like a good idea
//!
//! **Measured**, on one latent with two prompts as different from each other as
//! could be arranged (`backlog/image-generation-ideogram-4.md`):
//!
//! | prompt style | cos(A, B) | how much the prompt moves the answer |
//! |---|---|---|
//! | seven words | 0.9897 | 1.0% |
//! | structured JSON | 0.9667 | **3.3%** |
//!
//! Three times the effect for the same pair of ideas, written the way the model
//! expects. Ideogram 4 is conditioned on elaborate nested descriptions and a
//! bare phrase is far outside that — **the conditioning path is not broken; it
//! was being fed something the model was never trained on.**
//!
//! # And then it was measured properly, and the shape does nothing
//!
//! That table is **one latent**, and the effect varies by a factor of nineteen
//! between latents. Over eight
//! (`research/prompt-shape-does-nothing-2026-08-24.md`):
//!
//! | prompt style | mean effect | vs bare |
//! |---|---|---|
//! | bare phrase | 0.39% | 1.0x |
//! | **wrapped in an empty structured frame** | 0.36% | **0.9x** |
//! | written out by hand | 4.40% | **11.3x** |
//!
//! So the published claim was wrong twice: the real effect is **11.3x**, not
//! 3x — and **the JSON shape contributes nothing at all.** It is the sentences
//! that do the work: "soft even studio lighting from above, gentle shadow
//! beneath the apple", a named colour palette, a described background.
//!
//! # What this module is, given that
//!
//! **A form to fill in, and it says so.** `structure` returns the frame with
//! the descriptive fields *empty*, because a wrapper that invented *golden
//! hour, bokeh, 8k* would draw a different picture than the one asked for and
//! the user would have no way of knowing why.
//!
//! An empty form conditions no better than the phrase it wraps — that is the
//! measurement above — so **it is never applied silently and there is no button
//! that claims to improve a prompt.** `chaos-draw --template` prints it for a
//! person to fill in, which is the only honest thing to do with it.

/// Is this already the shape the model wants?
///
/// A cheap structural test rather than a parse: something that starts with `{`
/// and mentions the field the encoder keys on is already structured, and
/// wrapping it again would bury the description one level deeper.
pub fn is_structured(prompt: &str) -> bool {
    let t = prompt.trim_start();
    t.starts_with('{') && t.contains("high_level_description")
}

/// The frame, with `phrase` as its description and everything else blank.
///
/// **This is a form, not an improvement.** Handing it to the model as-is
/// conditions no better than the phrase alone — 0.9x, measured over eight
/// latents. It is worth something only once a person has written the lighting,
/// the background, the layout and a palette into it.
///
/// Returns the prompt unchanged if it is already structured.
pub fn structure(phrase: &str) -> String {
    let phrase = phrase.trim();
    if is_structured(phrase) {
        return phrase.to_string();
    }
    // The phrase is placed in JSON, so it has to be escaped. A quote in
    // "a photo of a "vintage" car" would otherwise end the string early and
    // produce something the encoder reads as a truncated description --
    // silently, because there is no parser on the way in.
    let described = escape(phrase);
    format!(
        r#"{{"high_level_description":"{described}","style_description":{{"aesthetics":"","lighting":"","photo":"high resolution, sharp focus","medium":"digital photograph","color_palette":[]}},"compositional_deconstruction":{{"canvas":"Square canvas, centred subject.","background":"","layout":"","elements":[{{"type":"obj","desc":"{described}"}}]}}}}"#
    )
}

/// A JSON string body: quotes, backslashes and control characters.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Anything else below space would be invalid JSON unescaped.
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phrase_becomes_a_frame_with_blanks_to_fill() {
        let s = structure("a red apple on a white table");
        assert!(is_structured(&s), "{s}");
        assert!(s.contains("a red apple on a white table"));
        assert!(s.starts_with('{') && s.ends_with('}'));
        // The frame is there for the user to fill, not filled for them.
        assert!(s.contains("\"aesthetics\":\"\""));
        assert!(s.contains("\"color_palette\":[]"));
    }

    /// **Scaffolding, not content.** A prompt "improver" that quietly adds
    /// *golden hour, bokeh, 8k* produces a different picture than the one that
    /// was asked for, and the user has no way of knowing why.
    #[test]
    fn nothing_is_invented() {
        let s = structure("a cat");
        for invented in [
            "golden hour",
            "bokeh",
            "8k",
            "masterpiece",
            "trending",
            "award",
            "cinematic",
        ] {
            assert!(
                !s.to_lowercase().contains(invented),
                "the wrapper invented {invented:?}"
            );
        }
    }

    /// Wrapping twice would bury the description a level deeper, where the
    /// model's conditioning would find it less rather than more.
    #[test]
    fn structuring_an_already_structured_prompt_changes_nothing() {
        let once = structure("a red apple");
        let twice = structure(&once);
        assert_eq!(once, twice);
        assert!(is_structured(&once));
        assert!(!is_structured("a red apple"));
        // Leading whitespace must not fool it either way.
        assert!(is_structured(&format!("  \n{once}")));
        assert!(!is_structured("{\"something_else\": 1}"));
    }

    /// **A quote in the phrase would end the JSON string early**, and there is
    /// no parser on the way in to notice — the encoder would simply be handed a
    /// truncated description.
    #[test]
    fn a_prompt_with_quotes_and_backslashes_survives() {
        let s = structure(r#"a photo of a "vintage" car in C:\garage"#);
        assert!(s.contains(r#"\"vintage\""#), "{s}");
        assert!(s.contains(r"C:\\garage"), "{s}");
        // It is still one JSON object: every quote is either a delimiter or
        // escaped, so the count of unescaped quotes stays even.
        let mut unescaped = 0;
        let bytes: Vec<char> = s.chars().collect();
        for (i, c) in bytes.iter().enumerate() {
            if *c == '"' && (i == 0 || bytes[i - 1] != '\\') {
                unescaped += 1;
            }
        }
        assert_eq!(unescaped % 2, 0, "an odd number of unescaped quotes: {s}");
    }

    #[test]
    fn control_characters_are_escaped_rather_than_passed_through() {
        let s = structure("line one\nline two\ttabbed");
        assert!(s.contains("\\n"), "{s}");
        assert!(s.contains("\\t"), "{s}");
        assert!(!s.contains('\n'), "a raw newline survived into the JSON");
    }

    #[test]
    fn an_empty_phrase_is_still_valid_json_shaped_text() {
        let s = structure("   ");
        assert!(is_structured(&s));
        assert!(s.starts_with('{') && s.ends_with('}'));
    }
}
