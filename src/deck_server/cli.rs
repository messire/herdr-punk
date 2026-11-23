//! `deck-server` has no command-line client, but the API schema shares its
//! request/param projection with Herdr's CLI (`src/cli/projection.rs`, the
//! per-domain `*bindings.rs` and their pure argument helpers). Those files
//! are compiled here unchanged; everything that talks to a server, a
//! terminal or a remote machine is left out.

#[path = "../cli/args.rs"]
mod args;
pub(crate) use args::*;

#[path = "../cli/projection.rs"]
pub(crate) mod projection;

#[path = "../cli/workspace_bindings.rs"]
pub(crate) mod workspace_bindings;

pub(crate) mod agent {
    #[path = "../../cli/agent/bindings.rs"]
    pub(crate) mod bindings;
}

pub(crate) mod pane {
    #[path = "../../cli/pane/bindings.rs"]
    pub(crate) mod bindings;
    #[path = "../../cli/pane/parse.rs"]
    mod parse;
    use parse::*;

    /// `pane <leaf> ARGS` as the API request Herdr's CLI would send, for the
    /// leaves agent hooks call through `$HERDR_BIN_PATH` (the hooks run
    /// `deck-server pane report-agent-session ...` inside deck-server panes).
    /// `None` for any other command.
    pub(crate) fn hook_request(
        leaf: &str,
        args: &[String],
    ) -> Option<Result<crate::api::schema::Request, String>> {
        const HOOK_LEAVES: &[&str] = &[
            "report-agent",
            "report-agent-session",
            "release-agent",
            "report-metadata",
        ];
        if !HOOK_LEAVES.contains(&leaf) {
            return None;
        }
        let found = bindings::LEAVES.iter().find(|l| l.name == leaf)?;
        Some(found.request(args).map_err(|err| format!("{err:?}")))
    }
}

pub(crate) mod plugin {
    #[path = "../../cli/plugin/bindings.rs"]
    pub(crate) mod bindings;

    /// Local form of `herdr plugin`'s path argument (no remote target here).
    fn normalize_plugin_path_arg(value: &str) -> std::io::Result<String> {
        super::absolute_local_path(value)
    }
}

pub(crate) mod worktree {
    #[path = "../../cli/worktree_bindings.rs"]
    pub(crate) mod bindings;

    /// Local form of `herdr worktree`'s path argument (no remote target here).
    fn normalize_path_arg(value: &str) -> std::io::Result<String> {
        super::absolute_local_path(value)
    }
}

/// The CLI's connection target. `deck-server` never runs as a CLI, so there
/// is no calling pane.
pub(crate) mod target {
    pub(crate) fn caller_pane_id() -> Option<String> {
        None
    }
}

fn absolute_local_path(value: &str) -> std::io::Result<String> {
    let path = crate::worktree::expand_tilde_path(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(absolute.display().to_string())
}

pub(crate) fn server_not_running_error(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
    )
}
