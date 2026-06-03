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

## Comment content: no narration, no history

A comment must explain the **why** behind a non-obvious choice — never narrate what the code does, nor record what it used to do. The following are banned:

- **Step narration** — comments describing the current step of a task or refactor ("now we update the state", "first parse, then validate", "wire this up"). The code already shows this.
- **Tombstones of old behavior** — comments documenting what the code did before ("previously this used X", "removed the old Y poller", "this replaces the legacy Z"). That belongs in git history, not in the source. *Exception:* keep one only when you judge it genuinely useful so a future agent does not forget a past mistake — e.g. "do NOT switch back to X here, it caused <known bug>". The point is the warning, not the nostalgia.
- **Restating the next line** in prose.

Write a comment **only** when it carries information the code cannot: a non-obvious invariant, the subtle reason for an unusual choice, or a warning that stops a future agent from reintroducing a known bug. Be very parsimonious — when in doubt, leave it out. When you encounter this kind of noise comment in code you are already editing, delete it.
