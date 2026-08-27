//! The brand pages: the burning grimoire, and the scrying circle that reads it.
//!
//! **One source of truth, four places it appears.** The art is authored as two
//! standalone HTML files under `assets/grimoire/`, and every tier of the
//! product serves or opens *those bytes* -- the server compiles them in with
//! `include_str!`, the desktop window opens the server's route, the phone
//! points a WebView at it. Nothing is re-implemented per platform, because a
//! second implementation is a second place for every fix to be missing from.
//! That rule is already written down about the forward pass in
//! `network/serve/src/lib.rs`; it is not weaker for a picture.
//!
//! **What this module adds to the raw files.** They are authored as document
//! *fragments* -- a `<title>`, a `<link>` to Google Fonts, then `<style>` and
//! the page -- because the tool they are previewed in supplies the skeleton.
//! A browser handed a fragment renders it in quirks mode, so [`mark`] and
//! [`scry`] wrap them in a real document. Two other things happen in the same
//! pass, and both matter more than the wrapper:
//!
//! - **The font link is replaced by the fonts themselves.** Chaos downloads
//!   nothing on its own. A page that reached out to `fonts.googleapis.com`
//!   would be the single exception, and it would fail exactly where it is most
//!   wanted: on a machine with no route out, and on a LAN-only node. The three
//!   faces are embedded as base64 WOFF2 in `assets/grimoire/fonts.css`
//!   (regenerate with `scripts/embed-fonts.py`; licences in
//!   `assets/grimoire/fonts/NOTICE`). A test asserts the assembled page has no
//!   fetchable external reference left in it.
//! - **The route is pushed in.** The mark encodes the address another machine
//!   uses to reach this node. Left to itself the page infers that from
//!   `location.origin`, which is right when a phone loaded it over the LAN and
//!   useless when the page was opened on the node's own loopback -- so the
//!   server, which knows what it bound to, hands the answer in through
//!   `window.CHAOS_ENDPOINT`.

/// The mark: a book bearing the Chaos sigil, burning inside a rune circle,
/// which opens to a QR code cut from this node's route.
pub const MARK: &str = include_str!("../../../assets/grimoire/grimoire.html");

/// The scrying circle: the same ring as a viewfinder, with a QR reader behind
/// it. It carries its own detector because `BarcodeDetector` is absent on
/// desktop Windows and on iOS.
pub const SCRY: &str = include_str!("../../../assets/grimoire/scanner.html");

/// Cinzel, IBM Plex Mono and UnifrakturMaguntia, latin subsets, base64 WOFF2.
pub const FONTS: &str = include_str!("../../../assets/grimoire/fonts.css");

/// Which of the two pages to build.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    /// The burning book, carrying this node's route.
    Mark,
    /// The reader.
    Scry,
}

/// What the host application knows and the page cannot work out for itself.
#[derive(Clone, Copy, Default, Debug)]
pub struct Host<'a> {
    /// The address another machine uses to reach this node, e.g.
    /// `http://192.168.1.20:8080`. `None` leaves the page to infer it.
    pub endpoint: Option<&'a str>,
    /// `Some("dark")` or `Some("light")` to stamp the theme on `<html>`, which
    /// is how the app makes the page match the window around it. `None` lets
    /// the operating system's preference decide, which is the page's default.
    pub theme: Option<&'a str>,
}

/// Build a complete, self-contained HTML document for one of the two pages.
pub fn page(which: Page, host: Host<'_>) -> String {
    let body = match which {
        Page::Mark => MARK,
        Page::Scry => SCRY,
    };
    let (title, rest) = split_fragment(body);

    // `data-theme` is what both pages already watch on the document element --
    // they observe it with a MutationObserver -- so stamping it here is the
    // supported way in rather than a new mechanism.
    let theme_attr = match host.theme {
        Some(t @ ("dark" | "light")) => format!(" data-theme=\"{t}\""),
        _ => String::new(),
    };

    let inject = match host.endpoint {
        Some(url) => format!(
            "<script>window.CHAOS_ENDPOINT={};</script>\n",
            js_string(url)
        ),
        None => String::new(),
    };

    // `viewport-fit=cover` because the circle is drawn to the full viewport and
    // a phone with a notch otherwise letterboxes it in the safe area.
    [
        "<!doctype html>\n<html lang=\"en\"",
        &theme_attr,
        ">\n<head>\n<meta charset=\"utf-8\">\n",
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">\n",
        "<meta name=\"color-scheme\" content=\"light dark\">\n",
        "<title>",
        &title,
        "</title>\n<style>\n",
        FONTS,
        "</style>\n",
        &inject,
        "</head>\n<body>\n",
        &rest,
        "\n</body>\n</html>\n",
    ]
    .concat()
}

/// Convenience for [`Page::Mark`].
pub fn mark(host: Host<'_>) -> String {
    page(Page::Mark, host)
}

/// Convenience for [`Page::Scry`].
pub fn scry(host: Host<'_>) -> String {
    page(Page::Scry, host)
}

/// Pull the `<title>` out of a fragment and drop its `<link>`s to Google Fonts.
///
/// Done by line rather than by parsing HTML, which is what the input actually
/// is: four single-line tags at the top of the file, then a `<style>` block.
/// A parser here would be a hundred lines defending against markup this
/// repository writes itself.
fn split_fragment(body: &str) -> (String, String) {
    let mut title = "Chaos".to_string();
    let mut kept: Vec<&str> = Vec::with_capacity(body.lines().count());
    for line in body.lines() {
        let t = line.trim();
        if let Some(inner) = t
            .strip_prefix("<title>")
            .and_then(|s| s.strip_suffix("</title>"))
        {
            title = inner.to_string();
            continue;
        }
        // Every `<link>` in these files points at fonts.googleapis.com or
        // fonts.gstatic.com, and the faces are embedded instead. The condition
        // is deliberately narrow: a link added later that is NOT a font would
        // survive this and fail the no-external-reference test, which is the
        // failure mode worth having.
        if t.starts_with("<link ") && t.contains("fonts.g") {
            continue;
        }
        kept.push(line);
    }
    (title, kept.join("\n").trim_start().to_string())
}

/// A JSON string literal, safe to paste inside a `<script>` element.
///
/// `<` is escaped as well as the JSON set: a payload containing `</script>`
/// would otherwise close the element early, and the endpoint is a value the
/// host application supplies rather than a constant.
fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The claim this module makes loudest, so it is the one under test:
    /// **the assembled page loads nothing from the network.**
    ///
    /// Checked against what a browser fetches *on its own* -- a stylesheet, a
    /// script, an image, a `url()`, an `@import`, a frame -- and deliberately
    /// not against the substring `http`, which appears in prose, in comments,
    /// and in the endpoint the page is *about*. An `<a href>` is excluded on
    /// purpose: it is a navigation the reader chooses, and the page has
    /// exactly one, covered by the test below.
    #[test]
    fn no_page_loads_anything_from_the_network() {
        for which in [Page::Mark, Page::Scry] {
            let html = page(which, Host::default());
            for needle in [
                "<link ",
                "<iframe",
                "src=\"http",
                "src='http",
                "src=\"//",
                "url(http",
                "url('http",
                "url(\"http",
                "@import",
                "fetch(\"http",
                "fonts.googleapis.com",
                "fonts.gstatic.com",
            ] {
                assert!(
                    !html.contains(needle),
                    "{which:?} would load something: found {needle:?}"
                );
            }
        }
    }

    /// The one absolute link either page contains, and what it is for.
    ///
    /// The mark shows where its code leads. Before a route is known that is
    /// the project's own repository -- the same string the QR encodes as its
    /// fallback -- and the script rewrites both to the live endpoint the
    /// moment there is one. Pinned here so a second outbound link cannot
    /// arrive unnoticed under the exclusion the test above makes for anchors.
    #[test]
    fn the_only_link_out_is_where_the_code_leads() {
        let mark = page(Page::Mark, Host::default());
        assert_eq!(mark.matches("href=\"http").count(), 1);
        assert!(mark.contains("href=\"https://github.com/aturzone/Chaos\""));
        assert_eq!(
            page(Page::Scry, Host::default())
                .matches("href=\"http")
                .count(),
            0
        );
    }

    #[test]
    fn the_fonts_are_actually_in_there() {
        let html = page(Page::Mark, Host::default());
        for family in ["Cinzel", "IBM Plex Mono", "UnifrakturMaguntia"] {
            assert!(
                html.contains(&format!("font-family:'{family}'")),
                "{family} is not embedded"
            );
        }
        assert!(html.contains("data:font/woff2;base64,"));
    }

    #[test]
    fn a_document_not_a_fragment() {
        for which in [Page::Mark, Page::Scry] {
            let html = page(which, Host::default());
            assert!(html.starts_with("<!doctype html>"));
            assert!(html.contains("<head>") && html.contains("<body>"));
            assert!(html.trim_end().ends_with("</html>"));
            // The title came from the fragment, not from the fallback.
            assert!(
                !html.contains("<title>Chaos</title>"),
                "{which:?} lost its title"
            );
            // And the fragment's own copy is gone from the body.
            assert_eq!(html.matches("<title>").count(), 1);
        }
    }

    #[test]
    fn the_route_is_pushed_in() {
        let html = page(
            Page::Mark,
            Host {
                endpoint: Some("http://192.168.1.20:8080"),
                theme: Some("dark"),
            },
        );
        assert!(html.contains("window.CHAOS_ENDPOINT=\"http://192.168.1.20:8080\";"));
        assert!(html.contains("<html lang=\"en\" data-theme=\"dark\">"));
    }

    /// A host that hands in a hostile endpoint must not be able to close the
    /// script element. This is not hypothetical politeness: the value can come
    /// from a command-line flag.
    #[test]
    fn an_endpoint_cannot_break_out_of_its_script() {
        let html = page(
            Page::Mark,
            Host {
                endpoint: Some("http://x/</script><script>alert(1)</script>"),
                theme: None,
            },
        );
        assert!(!html.contains("<script>alert(1)"));
        assert!(html.contains("\\u003c/script\\u003e"));
    }

    /// An unknown theme is ignored rather than stamped, so a typo cannot inject
    /// an attribute.
    #[test]
    fn only_the_two_themes_are_stamped() {
        let html = page(
            Page::Scry,
            Host {
                endpoint: None,
                theme: Some("\" onload=\"alert(1)"),
            },
        );
        assert!(html.contains("<html lang=\"en\">"));
        assert!(!html.contains("onload"));
    }

    /// The mark is the page that must keep working when the network changes;
    /// the scry page is the one that must say something useful without a
    /// camera. Both properties live in the HTML, so they are asserted here
    /// rather than trusted.
    #[test]
    fn the_pages_are_the_ones_we_think_they_are() {
        assert!(MARK.contains("resolveEndpoint"));
        assert!(MARK.contains("window.__grimoire"));
        assert!(SCRY.contains("getUserMedia"));
        assert!(SCRY.contains("window.__scry"));
    }
}
