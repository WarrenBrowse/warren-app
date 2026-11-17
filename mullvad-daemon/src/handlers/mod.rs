//! The `Daemon` command handlers, grouped by the settings or the domain
//! they act on. Each file is an `impl Daemon` block the dispatch in
//! `lib.rs` calls into.

mod access_method;
mod tunnel_options;
