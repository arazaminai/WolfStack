// Written by Paul Clevett
// (C)Copyright Wolf Software Systems Ltd
// https://wolf.uk.com

//! Legacy support metadata and sponsor flags.
//!
//! WolfStack is moving away from subscription-based access. This module now
//! only keeps local support state so older installs can be cleaned up while the
//! new one-off purchase flow takes over.

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

fn config_path() -> String { crate::paths::get().patreon_config }

/// Support tier levels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatreonTier {
    None,
    Free,
    Basic,
    Advanced,
    Platinum,
    Enterprise,
}

impl Default for PatreonTier {
    fn default() -> Self {
        PatreonTier::None
    }
}

impl PatreonTier {
    /// Whether this tier reflects an actual paid pledge — excludes `None`
    /// (not linked) and `Free` (follows on Patreon but pledges nothing).
    /// Drives BOTH the beta-channel grant and the login-time support-nag
    /// exemption: every paying backer is a supporter, so a $3 Basic pledge
    /// earns the same in-app perks (no nag, beta builds) as a higher tier.
    /// Tier *amount* differences are recognition only — the commercial value
    /// lives in licence-gated features (plugins, API tokens, SSO,
    /// multi-tenancy), never in donations.
    pub fn is_paying(&self) -> bool {
        matches!(self, PatreonTier::Basic | PatreonTier::Advanced | PatreonTier::Platinum | PatreonTier::Enterprise)
    }

    /// Determine tier from pledge amount in cents.
    pub fn from_cents(cents: i64) -> Self {
        if cents >= 9500 {
            PatreonTier::Platinum
        } else if cents >= 2500 {
            PatreonTier::Advanced
        } else if cents >= 300 {
            PatreonTier::Basic
        } else if cents > 0 {
            PatreonTier::Free
        } else {
            PatreonTier::None
        }
    }
}

/// Persisted legacy support config — stored at /etc/wolfstack/patreon.json.
/// The file name is historical; the active subscription plumbing has been
/// removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatreonConfig {
    #[serde(default)]
    pub tier: PatreonTier,
    #[serde(default)]
    pub pledge_amount_cents: i64,
    #[serde(default)]
    pub last_checked: Option<String>,
    #[serde(default)]
    pub linked: bool,
    /// Operator self-attests they support development via GitHub
    /// Sponsors at <https://github.com/sponsors/wolfsoftwaresystemsltd>.
    /// Honour-system — no OAuth verification (GitHub's Sponsors API
    /// requires the org's auth to enumerate sponsors, and the public
    /// sponsor listing is opt-in per sponsor). Beta access is granted
    /// to anyone who flips this; the gate is intentionally minimal
    /// because the cost of misuse is "stranger gets beta builds" and
    /// the cost of friction is "real sponsor can't unlock beta".
    #[serde(default)]
    pub github_sponsor: bool,
    /// Optional GitHub login for display purposes only. Not used as
    /// part of the access check. Lets the operator see (and prove to
    /// support) which account they linked.
    #[serde(default)]
    pub github_sponsor_login: Option<String>,
}

impl Default for PatreonConfig {
    fn default() -> Self {
        Self {
            tier: PatreonTier::None,
            pledge_amount_cents: 0,
            last_checked: None,
            linked: false,
            github_sponsor: false,
            github_sponsor_login: None,
        }
    }
}

impl PatreonConfig {
    pub fn load() -> Self {
        match std::fs::read_to_string(&config_path()) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        let dir = std::path::Path::new(&path).parent().unwrap();
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }
}

/// Runtime state held in AppState.
pub struct PatreonState {
    pub config: RwLock<PatreonConfig>,
}

impl PatreonState {
    pub fn new() -> Self {
        let config = PatreonConfig::load();
        Self {
            config: RwLock::new(config),
        }
    }

    /// Set the GitHub Sponsor self-attest flag and optional GitHub login.
    /// Persists immediately so the next process restart and any subsequent
    /// `/api/patreon/status` call see the new state.
    pub fn set_github_sponsor(&self, enabled: bool, login: Option<String>) -> Result<(), String> {
        let mut cfg = self.config.write().unwrap();
        cfg.github_sponsor = enabled;
        cfg.github_sponsor_login = login
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        // Save while holding the write lock so concurrent reads
        // never see a half-persisted state.
        cfg.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_from_cents() {
        assert_eq!(PatreonTier::from_cents(0), PatreonTier::None);
        assert_eq!(PatreonTier::from_cents(100), PatreonTier::Free);
        assert_eq!(PatreonTier::from_cents(300), PatreonTier::Basic);
        assert_eq!(PatreonTier::from_cents(2500), PatreonTier::Advanced);
        assert_eq!(PatreonTier::from_cents(9500), PatreonTier::Platinum);
        assert_eq!(PatreonTier::from_cents(20000), PatreonTier::Platinum);
    }

    #[test]
    fn test_is_paying() {
        // The support nag must NOT fire for anyone actually paying. The
        // critical boundary is Free (follows, pledges nothing) vs Basic
        // (first paid tier).
        assert!(!PatreonTier::None.is_paying());
        assert!(!PatreonTier::Free.is_paying());
        assert!(PatreonTier::Basic.is_paying());
        assert!(PatreonTier::Advanced.is_paying());
        assert!(PatreonTier::Platinum.is_paying());
        assert!(PatreonTier::Enterprise.is_paying());
    }
}
