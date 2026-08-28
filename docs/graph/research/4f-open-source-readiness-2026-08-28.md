---
topic: §4f — could a stranger take this, build it, and be legally clear
status: resolved — one real licence gap found and fixed, everything else already in place
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - 4e-production-readiness-2026-08-28.md
---

# §4f: open-source readiness

§4f asks about the licence and its compatibility with everything vendored —
**"including the fonts now embedded in the binary (OFL 1.1 … which the licence
requires be preserved and which a stripped build could lose)"** — plus
CONTRIBUTING, issue and PR templates, a build a stranger can reproduce, no
secrets in history, and CI that runs on a fork.

**The plan pointed straight at the one real problem.** Everything else was
already in place.

## The licence gap: the fonts were attributed nowhere a user receives

Three typefaces — **Cinzel**, **IBM Plex Mono**, **UnifrakturMaguntia** — are
under the SIL Open Font License 1.1 and are embedded as base64 WOFF2 in
`assets/grimoire/fonts.css`, which `chaos_arch::grimoire` `include_str!`s into
**every build**. So a Chaos binary contains the fonts, and so does every page it
serves at `/qr` and `/scan`.

The OFL requires the copyright notice to be preserved with the font. Measured:

| where a user might look | before |
|---|---|
| the root `NOTICE`, shipped in every archive | **0 mentions** of any of the three |
| the assembled page carrying the font bytes | **0 occurrences of "Copyright"** |
| `assets/grimoire/fonts/NOTICE` | complete and correct — **and not shipped** |

The attributions existed, in exactly one place: a file in the repository that the
release workflow does not copy. `release.yml` stages `README.md LICENSE NOTICE
CHANGELOG.md STATUS.md` from the root, so a person who downloads a release, or
loads `/qr` from someone's node, received OFL fonts with no attribution at all.

**A false lead worth recording.** A first pass grepped the emitted page for `OFL`
and found six matches, which looked like compliance. They were inside the base64
font blobs — three letters occurring by chance in megabytes of encoded binary.
Grepping for `Copyright` instead returned **zero**. A substring match on
base64 is not evidence of anything.

### Fixed in both places, and made a mechanism

- **The root `NOTICE`** now carries all three notices, states that the fonts are
  *embedded rather than linked*, says they are Google Fonts' own subsets
  byte-for-byte with no Reserved Font Name used for a modified version, and points
  at the full licence text.
- **Every assembled page** now carries a `FONT_NOTICE` comment before the
  `<style>` block that holds the fonts — so the notice travels with the bytes, to
  the browser, the desktop window, the phone and the APK's bundled copy.
- **`every_page_carries_the_fonts_licence`** asserts each page names the licence
  and all four copyright holders, and that the notice appears *before* the first
  `@font-face`. This is the half a minifier or a refactor could silently drop,
  which is precisely what §4f warned about.

Verified on the emitted files, which is what ships inside the APK:

```
qr.html / scan.html:  the attribution is present, and
                      <link=0, src="=0, @import=0, fetch(=0
```

Still fetch-free. The notice cost nothing.

### And a stale path in the same file

The root `NOTICE` described ggml's FFI as living in **`crates/chaos-ggml`** —
twice. That directory does not exist; it is `core/ggml`. It is the same hazard
§4c warns about (`crates/` is not the real tree) sitting in the one file whose
job is to be legally accurate. Fixed.

## Everything else was already in place

| §4f asks for | state |
|---|---|
| a licence | Apache-2.0, `LICENSE` at the root |
| third-party compatibility | ggml is MIT, linked as a prebuilt static library and **not vendored** — `NOTICE` says so and says why; OFL permits embedding |
| model weights | none distributed; `NOTICE` states they carry their own licences, which Chaos "neither grants nor alters" |
| CONTRIBUTING | present, and its stale test count was fixed earlier today — now machine-checked |
| issue templates | `.github/ISSUE_TEMPLATE/` — `bug_report.yml`, `feature_request.yml`, `config.yml` |
| PR template | `.github/PULL_REQUEST_TEMPLATE.md` |
| a build a stranger can reproduce | the README's steps *are* CI's steps — `ci.yml`'s own comment says it builds ggml from llama.cpp rather than vendoring it "the same steps a contributor follows by hand, so a green CI means the documented steps work" |
| **CI that runs on a fork** | **yes: `ci.yml` references `secrets.` zero times**, and triggers on `pull_request`. A fork's PR gets the full matrix with no secret to grant |
| no secrets in history | **0 commits** contain the token (searched with `git log --all -S`), and no file whose basename looks like a credential has ever been committed |

`release.yml` also references `secrets.` zero times — it relies on the
automatically-provided workflow token, so there is no secret for a maintainer to
rotate or a contributor to lack.

## What §4f leaves open

- **Nothing has verified that a stripped build keeps the notice.** The comment is
  in the HTML the binary carries, so `strip` on the executable does not touch it —
  but no one has run `strip` and re-read a served page. Cheap, not done.
- **The OFL's full text ships only via `assets/grimoire/fonts/NOTICE`**, which is
  in the repository and not in the archives. The root `NOTICE` names the licence
  and points at it. Whether a distributor considers that sufficient is a judgement
  call; including the 4,000-word licence in the root `NOTICE` would settle it and
  costs nothing but length. **Atur's call.**
- **§4e's uninstalled `.deb` and AppImage** are also a §4f concern: a distribution
  package nobody has installed is a licence-and-layout claim nobody has checked.
