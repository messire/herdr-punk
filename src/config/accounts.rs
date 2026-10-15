use serde::Deserialize;

pub(crate) const DEFAULT_ACCOUNTS_INTERVAL_SECONDS: u64 = 60;
pub(crate) const DEFAULT_ACCOUNTS_TIMEOUT_SECONDS: u64 = 20;
pub(crate) const MAX_ACCOUNTS_INTERVAL_SECONDS: u64 = 31_536_000;
pub(crate) const MAX_ACCOUNTS_TIMEOUT_SECONDS: u64 = 3_600;

/// Client-local "accounts" sidebar section fed by an external account manager
/// such as `cswap` (claude-swap). The section lives between Spaces and Agents.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AccountsConfig {
    /// Show the accounts section in the expanded desktop sidebar. Default: false.
    pub enabled: bool,
    /// Section title. Default: "accounts".
    pub title: String,
    /// Shell command that prints the account list as cswap-style JSON
    /// (`{"activeAccountNumber": 1, "accounts": [{"number", "email", "alias", "usage": ...}]}`).
    /// Default: "cswap list --json".
    pub list_command: String,
    /// Shell command run on double-click or the context menu. `{number}`, `{email}` and
    /// `{alias}` are replaced with the chosen account's values. Default: "cswap switch {number}".
    pub switch_command: String,
    /// Seconds between automatic refreshes. Default: 60.
    pub interval_seconds: u64,
    /// Seconds before a list or switch command is killed. Default: 20.
    pub timeout_seconds: u64,
}

impl Default for AccountsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            title: "accounts".into(),
            list_command: "cswap list --json".into(),
            switch_command: "cswap switch {number}".into(),
            interval_seconds: DEFAULT_ACCOUNTS_INTERVAL_SECONDS,
            timeout_seconds: DEFAULT_ACCOUNTS_TIMEOUT_SECONDS,
        }
    }
}

impl AccountsConfig {
    /// Effective polling interval after validation.
    pub(crate) fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.interval_seconds
                .clamp(1, MAX_ACCOUNTS_INTERVAL_SECONDS)
                .max(1),
        )
    }

    /// Effective command timeout after validation.
    pub(crate) fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.timeout_seconds
                .clamp(1, MAX_ACCOUNTS_TIMEOUT_SECONDS)
                .max(1),
        )
    }

    /// The switch command with account placeholders filled in.
    pub(crate) fn switch_command_for(&self, number: u64, email: &str, alias: &str) -> String {
        self.switch_command
            .replace("{number}", &number.to_string())
            .replace("{email}", email)
            .replace("{alias}", alias)
    }
}

pub(crate) fn accounts_diagnostics(config: &AccountsConfig) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if !config.enabled {
        return diagnostics;
    }
    if config.list_command.trim().is_empty() {
        diagnostics.push("ui.accounts.list_command is empty; hiding the accounts section".into());
    }
    if config.switch_command.trim().is_empty() {
        diagnostics
            .push("ui.accounts.switch_command is empty; account switching is disabled".into());
    }
    if config.interval_seconds < 1 || config.interval_seconds > MAX_ACCOUNTS_INTERVAL_SECONDS {
        diagnostics.push(format!(
            "ui.accounts.interval_seconds must be between 1 and {MAX_ACCOUNTS_INTERVAL_SECONDS}; using {DEFAULT_ACCOUNTS_INTERVAL_SECONDS}"
        ));
    }
    if config.timeout_seconds < 1 || config.timeout_seconds > MAX_ACCOUNTS_TIMEOUT_SECONDS {
        diagnostics.push(format!(
            "ui.accounts.timeout_seconds must be between 1 and {MAX_ACCOUNTS_TIMEOUT_SECONDS}; using {DEFAULT_ACCOUNTS_TIMEOUT_SECONDS}"
        ));
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_command_substitutes_placeholders() {
        let config = AccountsConfig {
            switch_command: "cswap switch {number} # {email} {alias}".into(),
            ..AccountsConfig::default()
        };
        assert_eq!(
            config.switch_command_for(2, "a@b.c", "work"),
            "cswap switch 2 # a@b.c work"
        );
    }

    #[test]
    fn disabled_section_reports_nothing() {
        let config = AccountsConfig {
            interval_seconds: 0,
            ..AccountsConfig::default()
        };
        assert!(accounts_diagnostics(&config).is_empty());
        let enabled = AccountsConfig {
            enabled: true,
            ..config
        };
        assert_eq!(accounts_diagnostics(&enabled).len(), 1);
    }
}
