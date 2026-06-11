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

## Typography: never use the em-dash (—)

**The em-dash character `—` (U+2014) is BANNED everywhere you author text in this repository.** This is a hard rule, not a stylistic preference. Never type, paste, or generate `—` in:

- User-facing strings and UI copy (source default strings in `.tsx`/`.swift`/`.kt`, etc.).
- Translation / i18n files in **every language**: `.po`, `.pot`, `.xcstrings`, Android `strings.xml`, and any other localization resource.
- Code comments, doc comments, and commit messages / PR descriptions.

When a sentence needs the kind of break an em-dash would provide, choose the natural alternative for the context:

- A **comma** (`,`) in the general case (use the language's own comma: `،` for Arabic, `，` for Chinese, `、` for Japanese).
- A **period** or a **colon** (`:`) when the clause is a full sentence or a label prefix (e.g. `WARNING: ...`). French keeps its space before the colon (` : `).
- A **hyphen** (`-`) for numeric or value ranges (e.g. `1-100`).
- **Nothing** (just a space, or restructure the sentence) when no punctuation is needed. In Thai, prefer a plain space.

Do **not** introduce the en-dash `–` (U+2013) as a substitute either; it is the same AI-typography tell.

**Rationale**: the em-dash reads as machine-generated boilerplate, is inconsistent with the rest of the copy, and was bulk-removed from the app once already. When you edit a file that still contains a stray `—`, replace it as part of your change (opportunistic cleanup).

## Comment content: no narration, no history

A comment must explain the **why** behind a non-obvious choice — never narrate what the code does, nor record what it used to do. The following are banned:

- **Step narration** — comments describing the current step of a task or refactor ("now we update the state", "first parse, then validate", "wire this up"). The code already shows this.
- **Tombstones of old behavior** — comments documenting what the code did before ("previously this used X", "removed the old Y poller", "this replaces the legacy Z"). That belongs in git history, not in the source. *Exception:* keep one only when you judge it genuinely useful so a future agent does not forget a past mistake — e.g. "do NOT switch back to X here, it caused <known bug>". The point is the warning, not the nostalgia.
- **Restating the next line** in prose.

Write a comment **only** when it carries information the code cannot: a non-obvious invariant, the subtle reason for an unusual choice, or a warning that stops a future agent from reintroducing a known bug. Be very parsimonious — when in doubt, leave it out. When you encounter this kind of noise comment in code you are already editing, delete it.

## Deployment rule: ALWAYS bump versions before redeploying exit nodes

Non-negotiable rule (poka, 2026-06-11). Before ANY redeploy of a warren-exit
binary to production (warren-exit-1, warren-exit-sin):

1. **Bump** `version` in `[workspace.package]` of `../warren-core/Cargo.toml`
   FIRST. Without it, two different builds carry the same number and only a
   SHA-256 hash comparison can tell what runs in prod.
2. **Commit before building**: never deploy a `-dirty` binary
   (`git describe` exposes it; `warren-exit --version` prints
   `git describe (semver)` since 2026-06-11).
3. **Verify after**: `ssh root@<exit> '/usr/local/bin/warren-exit --version'`
   must show the new version.
4. **Canary order**: warren-exit-sin first, then warren-exit-1.

Full procedure: `../warren-core/CLAUDE.md` section "Règles de déploiement des
exit nodes".
