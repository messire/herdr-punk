//! `deck-server`: the headless Herdr server alone, for Netrunner Deck.
//!
//! Derived from Herdr (Apache-2.0, see LICENSE and NOTICE). It is the same
//! server and JSON API as `herdr server` (same PROTOCOL_VERSION), built
//! without the TUI client, the CLI and the self-updater, and it keeps its own
//! config/state/session/socket directories (`deck-server`, `deck-server-dev`
//! for debug builds) so it can run next to a Herdr install.
//!
//! Usage: `deck-server [--session NAME]`.

// Modules shared with `herdr` carry code (and re-exports) only its TUI/CLI uses.
#![allow(dead_code, unused_imports)]

use std::io;

pub(crate) const HERDR_ENV_VAR: &str = "HERDR_ENV";
pub(crate) const HERDR_ENV_VALUE: &str = "1";

mod agent_resume;
mod agent_view_eval;
mod api;
mod app;
mod build_info;
#[path = "deck_server/cli.rs"]
mod cli;
#[path = "deck_server/client.rs"]
mod client;
mod config;
mod copy_mode;
mod detect;
mod events;
use ghostty_vt as ghostty;
mod handoff_runtime;
mod input;
mod integration;
mod ipc;
mod kitty_graphics;
mod layout;
mod logging;
mod metadata_tokens;
mod noninteractive_process;
mod pane;
use ghostty_vt::pane_graphics_files;
mod persist;
mod platform;
mod plugin_command;
mod plugin_installations;
mod plugin_paths;
mod popup_size;
mod product_announcements;
mod protocol;
mod pty;
mod raw_input;
mod release_notes;
mod render_prof;
mod render_signal;
mod selection;
mod server;
mod session;
mod sound;
mod terminal;
mod terminal_notify;
mod terminal_theme;
mod thread_spawn;
mod ui;
#[path = "deck_server/update.rs"]
mod update;
mod workspace;
mod worktree;

/// Config, state, sessions and the API/client sockets live under this name.
const APP_DIR_NAME: &str = if cfg!(debug_assertions) {
    "deck-server-dev"
} else {
    "deck-server"
};

/// Herdr variables a parent Herdr pane exports. Inherited, they would point
/// this server at Herdr's own sockets, config or session.
const INHERITED_HERDR_VARS: &[&str] = &[
    api::SOCKET_PATH_ENV_VAR,
    server::socket_paths::CLIENT_SOCKET_PATH_ENV_VAR,
    config::CONFIG_PATH_ENV_VAR,
    session::SESSION_ENV_VAR,
    HERDR_ENV_VAR,
];

fn main() -> io::Result<()> {
    for var in INHERITED_HERDR_VARS {
        std::env::remove_var(var);
    }
    config::set_app_dir_name(APP_DIR_NAME);
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .skip(1)
        .any(|a| a == "--help" || a == "-h" || a == "--version" || a == "-V")
    {
        println!(
            "deck-server {} (herdr protocol {})\nusage: deck-server [--session NAME]\napi socket: {}",
            env!("CARGO_PKG_VERSION"),
            protocol::PROTOCOL_VERSION,
            api::socket_path().display()
        );
        return Ok(());
    }
    // `--session NAME` selects `sessions/NAME` under the app dir, as in Herdr.
    if let Err(err) = session::configure_from_args(&args) {
        eprintln!("error: {err}");
        std::process::exit(2);
    }
    server::headless::run_server()
}
