//! Client-local "accounts" sidebar section: a list of credential slots managed
//! by an external tool (cswap / claude-swap by default), refreshed from its JSON
//! output on a background thread and switched through the same tool.
//!
//! Everything here is TUI presentation state. Nothing touches the server, the
//! snapshot, or the wire protocol.

use std::{
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use crate::config::AccountsConfig;

/// One credential slot as the sidebar shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClientAccount {
    pub(crate) number: u64,
    pub(crate) email: String,
    pub(crate) alias: Option<String>,
    pub(crate) active: bool,
    pub(crate) disabled: bool,
    /// Five-hour window usage, 0–100.
    pub(crate) five_hour_pct: Option<u8>,
    /// Seven-day window usage, 0–100.
    pub(crate) seven_day_pct: Option<u8>,
    /// Time left until the five-hour window resets, as the tool prints it ("4h 55m").
    pub(crate) five_hour_reset: Option<String>,
    /// Time left until the seven-day window resets.
    pub(crate) seven_day_reset: Option<String>,
    /// Per-model windows: (label, pct, reset countdown).
    pub(crate) scoped: Vec<(String, Option<u8>, Option<String>)>,
    /// Tool-reported status other than "ok", e.g. "relogin_required".
    pub(crate) status: Option<String>,
}

impl ClientAccount {
    /// Sidebar lines this account needs: title, 5h, 7d, one per model window.
    pub(crate) fn line_count(&self) -> usize {
        3 + self.scoped.len()
    }

    /// Short badge for a non-ok tool status.
    pub(crate) fn status_badge(&self) -> Option<&'static str> {
        match self.status.as_deref()? {
            "ok" => None,
            "relogin_required" | "relogin" | "expired" => Some("relogin"),
            "rate_limited" | "limited" => Some("limited"),
            "error" | "fetch_failed" => Some("error"),
            _ => Some("!"),
        }
    }

    /// Alias when set, otherwise the mailbox part of the email.
    pub(crate) fn label(&self) -> &str {
        if let Some(alias) = self.alias.as_deref().filter(|alias| !alias.is_empty()) {
            return alias;
        }
        self.email
            .split_once('@')
            .map(|(user, _)| user)
            .filter(|user| !user.is_empty())
            .unwrap_or(&self.email)
    }
}

/// Latest poll result the sidebar renders.
#[derive(Clone, Debug, Default)]
pub(crate) struct ClientAccountsState {
    pub(crate) accounts: Vec<ClientAccount>,
    pub(crate) error: Option<String>,
    pub(crate) fetched_at: Option<Instant>,
    /// A refresh or switch is in flight.
    pub(crate) busy: bool,
}

/// What the shell asks the poller thread to do.
#[derive(Debug)]
pub(crate) enum AccountsRequest {
    Refresh,
    /// Run this shell command, then refresh.
    Switch(String),
    Reconfigure(AccountsConfig),
}

/// Result delivered back to the client loop.
pub(crate) type AccountsPollResult = Result<Vec<ClientAccount>, String>;

/// Starts the poller and returns the control channel the shell keeps.
pub(crate) fn spawn_poller(
    config: AccountsConfig,
    deliver: impl Fn(AccountsPollResult) -> bool + Send + 'static,
    should_quit: Arc<AtomicBool>,
) -> mpsc::Sender<AccountsRequest> {
    let (tx, rx) = mpsc::channel::<AccountsRequest>();
    std::thread::Builder::new()
        .name("herdr-accounts".into())
        .spawn(move || poll_loop(config, rx, deliver, should_quit))
        .map(|_| ())
        .unwrap_or_else(
            |err| tracing::warn!(error = %err, "failed to start the accounts poller thread"),
        );
    tx
}

fn poll_loop(
    mut config: AccountsConfig,
    rx: mpsc::Receiver<AccountsRequest>,
    deliver: impl Fn(AccountsPollResult) -> bool,
    should_quit: Arc<AtomicBool>,
) {
    let mut next_poll = Instant::now();
    loop {
        if should_quit.load(Ordering::Acquire) {
            return;
        }
        let now = Instant::now();
        let wait = next_poll.saturating_duration_since(now);
        let request = match rx.recv_timeout(wait) {
            Ok(request) => Some(request),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        match request {
            Some(AccountsRequest::Reconfigure(next)) => {
                let changed = next != config;
                config = next;
                if !changed {
                    continue;
                }
            }
            Some(AccountsRequest::Switch(command)) => {
                if let Err(error) = run_command(&command, config.timeout()) {
                    if !deliver(Err(error)) {
                        return;
                    }
                    next_poll = Instant::now() + config.interval();
                    continue;
                }
            }
            Some(AccountsRequest::Refresh) | None => {}
        }
        if !config.enabled || config.list_command.trim().is_empty() {
            next_poll = Instant::now() + config.interval();
            continue;
        }
        let result = run_command(&config.list_command, config.timeout())
            .and_then(|output| parse_accounts_json(&output));
        if !deliver(result) {
            return;
        }
        next_poll = Instant::now() + config.interval();
    }
}

/// Runs a shell command, returning its stdout or a one-line error.
fn run_command(command: &str, timeout: Duration) -> Result<String, String> {
    let mut process = crate::platform::detached_custom_command_process(command);
    process
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = process
        .spawn()
        .map_err(|err| format!("failed to start `{command}`: {err}"))?;
    let stdout_reader = drain(child.stdout.take());
    let stderr_reader = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(err) => {
                let _ = child.kill();
                return Err(format!("`{command}` failed: {err}"));
            }
        }
    };
    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    let Some(status) = status else {
        return Err(format!(
            "`{command}` timed out after {}s",
            timeout.as_secs()
        ));
    };
    if !status.success() {
        let detail = stderr
            .lines()
            .chain(stdout.lines())
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("no output");
        return Err(format!("`{command}` exited with {status}: {detail}"));
    }
    Ok(stdout)
}

/// Reads a child pipe to the end on its own thread so a chatty command never
/// blocks on a full pipe while the parent waits for it.
fn drain<R: std::io::Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_string(&mut text);
        }
        text
    })
}

/// Parses `cswap list --json`. Tolerates leading noise before the JSON object.
pub(crate) fn parse_accounts_json(text: &str) -> AccountsPollResult {
    let start = text
        .find('{')
        .ok_or_else(|| "list command printed no JSON".to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text[start..])
        .map_err(|err| format!("list command printed invalid JSON: {err}"))?;
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| error.to_string());
        return Err(message);
    }
    let active_number = value.get("activeAccountNumber").and_then(|v| v.as_u64());
    let accounts = value
        .get("accounts")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "list command JSON has no `accounts` array".to_string())?;
    let mut parsed = Vec::with_capacity(accounts.len());
    for (index, account) in accounts.iter().enumerate() {
        let number = account
            .get("number")
            .and_then(|v| v.as_u64())
            .unwrap_or(index as u64 + 1);
        let email = account
            .get("email")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let alias = account
            .get("alias")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|alias| !alias.is_empty())
            .map(str::to_string);
        let active = account
            .get("active")
            .and_then(|v| v.as_bool())
            .unwrap_or(active_number == Some(number));
        let disabled = account
            .get("disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let usage = account
            .get("usage")
            .filter(|usage| usage.is_object())
            .or_else(|| account.get("lastGoodUsage"));
        let pct_of = |window: &serde_json::Value| {
            window
                .get("pct")
                .or_else(|| window.get("utilization"))
                .and_then(|pct| pct.as_f64())
                .map(normalize_pct)
        };
        let reset_of = |window: &serde_json::Value| {
            window
                .get("countdown")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        let window = |name: &str| usage.and_then(|usage| usage.get(name));
        let scoped = usage
            .and_then(|usage| usage.get("scoped"))
            .and_then(|v| v.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .map(|entry| {
                        let name = entry
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("model")
                            .to_string();
                        (name, pct_of(entry), reset_of(entry))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let status = account
            .get("usageStatus")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        parsed.push(ClientAccount {
            number,
            email,
            alias,
            active,
            disabled,
            five_hour_pct: window("fiveHour").and_then(pct_of),
            seven_day_pct: window("sevenDay").and_then(pct_of),
            five_hour_reset: window("fiveHour").and_then(reset_of),
            seven_day_reset: window("sevenDay").and_then(reset_of),
            scoped,
            status,
        });
    }
    Ok(parsed)
}

/// cswap reports percentages; a fraction in `0..=1` with a fractional part is
/// treated as a ratio so both spellings land on the same 0–100 scale.
fn normalize_pct(value: f64) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

/// Five-cell usage bar: `▰▰▱▱▱`.
pub(crate) fn usage_bar(pct: Option<u8>) -> String {
    const WIDTH: usize = 5;
    match pct {
        None => "▱".repeat(WIDTH),
        Some(pct) => {
            let used = ((f64::from(pct) / 100.0) * WIDTH as f64).round() as usize;
            let used = used.min(WIDTH);
            format!("{}{}", "▰".repeat(used), "▱".repeat(WIDTH - used))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cswap_list_json() {
        let text = r#"warming up
{"schemaVersion":1,"activeAccountNumber":1,"accounts":[
  {"number":1,"email":"work@plurio.ai","active":true,"usage":{"fiveHour":{"pct":2},"sevenDay":{"pct":46}}},
  {"number":2,"email":"me@proton.me","alias":"personal","active":false,"disabled":true,"usageStatus":"relogin_required","usage":null,"lastGoodUsage":{"fiveHour":{"pct":31},"sevenDay":{"pct":21,"countdown":"2d 1h"},"scoped":[{"name":"Fable","pct":17,"countdown":"2d 1h"}]}}
]}"#;
        let accounts = parse_accounts_json(text).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].label(), "work");
        assert!(accounts[0].active);
        assert_eq!(accounts[0].five_hour_pct, Some(2));
        assert_eq!(accounts[1].label(), "personal");
        assert!(accounts[1].disabled);
        assert_eq!(accounts[1].five_hour_pct, Some(31));
        assert_eq!(accounts[1].seven_day_pct, Some(21));
        assert_eq!(accounts[1].seven_day_reset.as_deref(), Some("2d 1h"));
        assert_eq!(accounts[1].scoped.len(), 1);
        assert_eq!(accounts[1].scoped[0].0, "Fable");
        assert_eq!(accounts[1].scoped[0].1, Some(17));
        assert_eq!(accounts[1].status_badge(), Some("relogin"));
        assert_eq!(accounts[1].line_count(), 4);
        assert_eq!(accounts[0].status_badge(), None);
    }

    #[test]
    fn surfaces_tool_errors() {
        let err = parse_accounts_json(r#"{"schemaVersion":1,"error":{"message":"no accounts"}}"#)
            .unwrap_err();
        assert_eq!(err, "no accounts");
        assert!(parse_accounts_json("nothing here").is_err());
    }

    #[test]
    fn usage_bar_scales_to_five_cells() {
        assert_eq!(usage_bar(None), "▱▱▱▱▱");
        assert_eq!(usage_bar(Some(0)), "▱▱▱▱▱");
        assert_eq!(usage_bar(Some(50)), "▰▰▰▱▱");
        assert_eq!(usage_bar(Some(38)), "▰▰▱▱▱");
        assert_eq!(usage_bar(Some(100)), "▰▰▰▰▰");
        assert_eq!(usage_bar(Some(90)), "▰▰▰▰▱");
    }
}

impl super::ClientShellState {
    /// The accounts section settings the poller should run with.
    pub(crate) fn accounts_config(&self) -> AccountsConfig {
        self.config.accounts.clone()
    }

    /// Hands the shell the poller's control channel and asks for a first list.
    pub(crate) fn set_accounts_control(&mut self, control: mpsc::Sender<AccountsRequest>) {
        self.accounts_control = Some(control);
        self.request_accounts_refresh();
    }

    /// Stores a poll result. Returns true when the sidebar needs a repaint.
    pub(crate) fn apply_accounts_result(&mut self, result: AccountsPollResult) -> bool {
        self.accounts.busy = false;
        self.accounts.fetched_at = Some(Instant::now());
        match result {
            Ok(accounts) => {
                let changed = accounts != self.accounts.accounts || self.accounts.error.is_some();
                self.accounts.accounts = accounts;
                self.accounts.error = None;
                changed || self.config.accounts.enabled
            }
            Err(error) => {
                let changed = self.accounts.error.as_deref() != Some(error.as_str());
                if changed && !self.accounts.accounts.is_empty() {
                    // The list stays on screen; the failure itself goes through the notice bar.
                    self.set_endpoint_error(error.clone());
                }
                self.accounts.error = Some(error);
                changed || self.config.accounts.enabled
            }
        }
    }

    pub(super) fn request_accounts_refresh(&mut self) {
        if let Some(control) = self.accounts_control.as_ref() {
            if control.send(AccountsRequest::Refresh).is_ok() {
                self.accounts.busy = true;
            }
        }
    }

    /// Runs the configured switch command for the account at `index`, then refreshes.
    pub(super) fn request_account_switch(&mut self, index: usize) {
        let Some(account) = self.accounts.accounts.get(index) else {
            return;
        };
        if account.active {
            return;
        }
        let command = self.config.accounts.switch_command_for(
            account.number,
            &account.email,
            account.alias.as_deref().unwrap_or_default(),
        );
        if command.trim().is_empty() {
            self.set_endpoint_error("ui.accounts.switch_command is empty");
            return;
        }
        match self.accounts_control.as_ref() {
            Some(control) if control.send(AccountsRequest::Switch(command)).is_ok() => {
                self.accounts.busy = true;
            }
            _ => self.set_endpoint_error("accounts poller is not running"),
        }
    }

    /// Pushes the current config to the poller after a live reload.
    pub(super) fn reconfigure_accounts(&mut self) {
        if let Some(control) = self.accounts_control.as_ref() {
            let _ = control.send(AccountsRequest::Reconfigure(self.config.accounts.clone()));
        }
        if !self.config.accounts.enabled {
            self.accounts = ClientAccountsState::default();
        }
    }

    pub(super) fn toggle_section(&mut self, section: super::SidebarSection) {
        if !self.collapsed_sections.remove(&section) {
            self.collapsed_sections.insert(section);
        }
        match section {
            super::SidebarSection::Spaces => self.workspace_scroll = 0,
            super::SidebarSection::Accounts => self.account_scroll = 0,
            super::SidebarSection::Agents => self.agent_scroll = 0,
        }
        self.invalidate_pane_surface();
    }

    pub(super) fn section_collapsed(&self, section: super::SidebarSection) -> bool {
        self.collapsed_sections.contains(&section)
    }
}
