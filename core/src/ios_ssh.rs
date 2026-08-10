//! iOS jailbreak SSH helpers — PATH injection and non-interactive privilege
//! escalation for `root` / `mobile` sessions.
//!
//! Jailbroken iPhones typically run OpenSSH or dropbear with a minimal login
//! `PATH` (no `/var/jb/...`), and many ops started as `mobile` need `su`/`sudo`
//! to reach root. Callers attach an [`ExecProfile`] to a live session; every
//! remote command is wrapped before it hits the SSH exec channel.

use crate::model::SavedConnection;

/// Jailbreak-friendly PATH prefix (rootless Procursus + legacy binpack).
pub const IOS_JAILBREAK_PATH: &str =
    "/var/jb/usr/bin:/var/jb/bin:/var/jb/usr/sbin:/var/jb/sbin:/iosbinpack64/usr/bin:/iosbinpack64/bin:/usr/bin:/bin:/usr/sbin:/sbin";

/// How (if at all) to escalate before running a remote command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElevateMethod {
    #[default]
    None,
    /// `su root -c '…'` with password on stdin (classic iOS OpenSSH).
    Su,
    /// `sudo -S …` with password on stdin.
    Sudo,
}

impl ElevateMethod {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "su" => Self::Su,
            "sudo" => Self::Sudo,
            _ => Self::None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Su => "su",
            Self::Sudo => "sudo",
        }
    }
}

/// Per-session remote-exec policy derived from a saved connection (or MCP params).
#[derive(Debug, Clone, Default)]
pub struct ExecProfile {
    /// When true, prepend the jailbreak PATH before every command.
    pub ios: bool,
    pub elevate: ElevateMethod,
    /// Password fed to `su` / `sudo -S`. Usually the connection password.
    pub elevate_password: Option<String>,
}

impl ExecProfile {
    /// Build a profile from a saved connection.
    ///
    /// Tags (GUI-friendly, no schema migration):
    /// - `platform:ios` or bare `ios` → enable PATH wrap
    /// - `elevate:su` / `elevate:sudo` → privilege escalation
    pub fn from_connection(connection: &SavedConnection) -> Self {
        let ios = connection
            .tags
            .iter()
            .any(|t| matches_tag(t, "platform:ios") || matches_tag(t, "ios"))
            || connection.protocol.eq_ignore_ascii_case("ios");

        let elevate = connection
            .tags
            .iter()
            .find_map(|t| {
                let lower = t.trim().to_ascii_lowercase();
                lower
                    .strip_prefix("elevate:")
                    .map(ElevateMethod::parse)
                    .or_else(|| {
                        if lower == "elevate" {
                            Some(ElevateMethod::Su)
                        } else {
                            None
                        }
                    })
            })
            .unwrap_or(ElevateMethod::None);

        let elevate_password = connection
            .password
            .clone()
            .filter(|p| !p.is_empty());

        Self {
            ios,
            elevate,
            elevate_password,
        }
    }

    /// True when any wrapping will be applied.
    pub fn needs_wrap(&self) -> bool {
        self.ios || self.elevate != ElevateMethod::None
    }
}

fn matches_tag(tag: &str, expected: &str) -> bool {
    tag.trim().eq_ignore_ascii_case(expected)
}

/// Single-quote a string for POSIX `sh` (safe inside `su -c '…'`).
pub fn shell_single_quote(input: &str) -> String {
    // foo → 'foo';  it's → 'it'\''s'
    let mut out = String::with_capacity(input.len() + 2);
    out.push('\'');
    for (i, part) in input.split('\'').enumerate() {
        if i > 0 {
            out.push_str("'\\''");
        }
        out.push_str(part);
    }
    out.push('\'');
    out
}

/// Wrap `command` according to `profile` so it is safe to pass to SSH `exec`.
///
/// Order: elevate(inner) → PATH export around the whole thing when `ios`.
/// The result is always a single string suitable for `sh -c`-style exec.
pub fn wrap_command(command: &str, profile: &ExecProfile) -> String {
    if !profile.needs_wrap() {
        return command.to_string();
    }

    let elevated = match profile.elevate {
        ElevateMethod::None => command.to_string(),
        ElevateMethod::Su => {
            let quoted = shell_single_quote(command);
            // Jailbreak iOS: interactive `su -` often errors with
            // "failed to create session", and non-interactive `su` reads the
            // password from /dev/tty (ignores stdin pipes) → MCP exec hangs
            // forever. On iOS, elevate:su is therefore implemented with sudo -S.
            if profile.ios {
                match profile.elevate_password.as_deref() {
                    Some(pass) if !pass.is_empty() => {
                        let pass_q = shell_single_quote(pass);
                        format!("printf '%s\\n' {pass_q} | sudo -S -p '' sh -c {quoted}")
                    }
                    _ => format!("sudo -n sh -c {quoted}"),
                }
            } else {
                match profile.elevate_password.as_deref() {
                    Some(pass) if !pass.is_empty() => {
                        let pass_q = shell_single_quote(pass);
                        format!("printf '%s\\n' {pass_q} | su root -c {quoted}")
                    }
                    _ => format!("su root -c {quoted}"),
                }
            }
        }
        ElevateMethod::Sudo => {
            let quoted = shell_single_quote(command);
            match profile.elevate_password.as_deref() {
                Some(pass) if !pass.is_empty() => {
                    let pass_q = shell_single_quote(pass);
                    // -S read password from stdin; -p '' suppress prompt noise.
                    format!("printf '%s\\n' {pass_q} | sudo -S -p '' sh -c {quoted}")
                }
                _ => format!("sudo -n sh -c {quoted}"),
            }
        }
    };

    if profile.ios {
        format!("export PATH={IOS_JAILBREAK_PATH}:$PATH; {elevated}")
    } else {
        elevated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ConnectionStatus, SavedConnection};

    fn conn(tags: &[&str], password: Option<&str>) -> SavedConnection {
        SavedConnection {
            id: "ssh-test".into(),
            name: "ios".into(),
            host: "127.0.0.1".into(),
            port: 22,
            username: "mobile".into(),
            protocol: "SSH".into(),
            folder: "All Connections".into(),
            tags: tags.iter().map(|s| (*s).to_string()).collect(),
            description: String::new(),
            auth_method: "password".into(),
            password: password.map(str::to_string),
            private_key_path: None,
            passphrase: None,
            status: ConnectionStatus::Disconnected,
        }
    }

    #[test]
    fn profile_from_ios_tags() {
        let p = ExecProfile::from_connection(&conn(&["platform:ios", "elevate:su"], Some("alpine")));
        assert!(p.ios);
        assert_eq!(p.elevate, ElevateMethod::Su);
        assert_eq!(p.elevate_password.as_deref(), Some("alpine"));
    }

    #[test]
    fn bare_ios_tag_counts() {
        let p = ExecProfile::from_connection(&conn(&["ios"], None));
        assert!(p.ios);
        assert_eq!(p.elevate, ElevateMethod::None);
    }

    #[test]
    fn wrap_injects_path_only() {
        let profile = ExecProfile {
            ios: true,
            elevate: ElevateMethod::None,
            elevate_password: None,
        };
        let wrapped = wrap_command("apt update", &profile);
        assert!(wrapped.starts_with("export PATH="));
        assert!(wrapped.contains(IOS_JAILBREAK_PATH));
        assert!(wrapped.ends_with("apt update"));
    }

    #[test]
    fn wrap_su_on_ios_uses_sudo() {
        let profile = ExecProfile {
            ios: true,
            elevate: ElevateMethod::Su,
            elevate_password: Some("alpine".into()),
        };
        let wrapped = wrap_command("id", &profile);
        // iOS must not pipe into `su` (tty hang / failed to create session).
        assert!(wrapped.contains("sudo -S"));
        assert!(!wrapped.contains("su root -c"));
        assert!(wrapped.contains("printf"));
        assert!(wrapped.contains("alpine"));
        assert!(wrapped.contains(IOS_JAILBREAK_PATH));
    }

    #[test]
    fn wrap_su_on_linux_keeps_su() {
        let profile = ExecProfile {
            ios: false,
            elevate: ElevateMethod::Su,
            elevate_password: Some("secret".into()),
        };
        let wrapped = wrap_command("id", &profile);
        assert!(wrapped.contains("su root -c"));
        assert!(!wrapped.contains("sudo -S"));
    }

    #[test]
    fn shell_quote_handles_apostrophe() {
        let q = shell_single_quote("it's");
        // POSIX: 'it'\''s'
        assert_eq!(q, "'it'\\''s'");
    }

    #[test]
    fn no_wrap_passthrough() {
        let profile = ExecProfile::default();
        assert_eq!(wrap_command("echo hi", &profile), "echo hi");
    }
}
