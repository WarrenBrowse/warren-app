# Warren App — Project Rules for Claude Code

## Language policy

### Code comments: English only

**ALL code comments in this repository MUST be written in English.** This applies to every commentable file in the codebase, including but not limited to:

- Rust (`.rs`) — `//`, `///`, `//!`, `/* */`
- TypeScript / JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`) — `//`, `/* */`, JSDoc `/** */`
- Swift (`.swift`), Kotlin (`.kt`), Java (`.java`)
- Shell scripts (`.sh`, `.bash`, `.zsh`) — `#`
- TOML (`Cargo.toml`, etc.), YAML (`.yaml`, `.yml`) — `#`
- Python (`.py`) — `#`, docstrings
- Dockerfile, Makefile, and any other commentable config

This includes:
- Inline comments
- Doc comments (rustdoc, JSDoc, Swift doc comments, etc.)
- Module-level documentation
- Test descriptions inside comments
- TODO / FIXME / NOTE markers
- Block comments and headers

**Rationale**: Warren is a fork of Mullvad VPN (English-only upstream). Keeping comments uniformly in English:
- Makes upstream rebases cleaner (no diff noise from translated comments).
- Keeps the codebase accessible to non-French-speaking contributors and reviewers.
- Aligns the fork's code with the upstream conventions.

### Exceptions (NOT code comments)

The English-only rule does **not** apply to:
- `.planning/` artifacts (internal planning docs may be in French).
- Markdown documentation written specifically for the French-speaking Warren team (clearly scoped).
- User-facing strings, UI translations, and i18n files (these have their own translation flow).
- Git commit messages and PR descriptions (no enforced language, but English is preferred for upstream alignment).
- Assistant chat / conversation output to the user (stays in French per user preference).

### When in doubt

If you are about to write a comment in any source/config file (`src/`, `Cargo.toml`, etc.), write it in English. If you are unsure whether a file counts as "code" or "planning artifact", check the path: anything under `.planning/`, `docs/` written for the Warren team, or explicitly French-tagged is allowed in French; everything else must be English.

### Translating existing French comments

When you encounter a French comment in code while making unrelated changes, translate it to English as part of the change (opportunistic cleanup). Do not introduce new French comments.
