//! Browser network policy.
//!
//! Profile policy declares whether arbitrary browsing is allowed or confined to
//! an approved-domain allowlist. The default is deny-by-default: a browser
//! specialist with no explicit policy reaches nothing. An allowlist with no
//! domains is a misconfiguration (it permits nothing while claiming to permit
//! something) and fails readiness.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    /// No outbound browsing at all.
    DenyAll,
    /// Browsing limited to `allowed_domains` (and their subdomains).
    Allowlist,
    /// Arbitrary browsing permitted.
    Open,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub mode: NetworkMode,
    pub allowed_domains: Vec<String>,
}

impl NetworkPolicy {
    pub fn deny_all() -> Self {
        Self {
            mode: NetworkMode::DenyAll,
            allowed_domains: Vec::new(),
        }
    }

    pub fn allowlist(domains: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            mode: NetworkMode::Allowlist,
            allowed_domains: domains.into_iter().map(Into::into).collect(),
        }
    }

    pub fn open() -> Self {
        Self {
            mode: NetworkMode::Open,
            allowed_domains: Vec::new(),
        }
    }

    /// Whether a host is reachable under this policy. Allowlist entries match
    /// the exact host or any subdomain of it.
    pub fn is_host_allowed(&self, host: &str) -> bool {
        match self.mode {
            NetworkMode::DenyAll => false,
            NetworkMode::Open => true,
            NetworkMode::Allowlist => self.allowed_domains.iter().any(|d| {
                host == d || host.ends_with(&format!(".{d}"))
            }),
        }
    }

    /// Whether the policy is internally coherent enough to apply. An allowlist
    /// must name at least one domain; deny-all and open are always coherent.
    pub fn resolves(&self) -> bool {
        match self.mode {
            NetworkMode::DenyAll | NetworkMode::Open => true,
            NetworkMode::Allowlist => !self.allowed_domains.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_all_blocks_everything_but_resolves() {
        let p = NetworkPolicy::deny_all();
        assert!(!p.is_host_allowed("example.com"));
        assert!(p.resolves());
    }

    #[test]
    fn open_allows_everything() {
        let p = NetworkPolicy::open();
        assert!(p.is_host_allowed("anything.example.org"));
        assert!(p.resolves());
    }

    #[test]
    fn allowlist_matches_exact_and_subdomains() {
        let p = NetworkPolicy::allowlist(["example.com"]);
        assert!(p.is_host_allowed("example.com"));
        assert!(p.is_host_allowed("docs.example.com"));
        assert!(!p.is_host_allowed("evil.com"));
        assert!(!p.is_host_allowed("notexample.com"));
        assert!(p.resolves());
    }

    #[test]
    fn empty_allowlist_does_not_resolve() {
        let p = NetworkPolicy::allowlist(Vec::<String>::new());
        assert!(!p.resolves());
        assert!(!p.is_host_allowed("example.com"));
    }
}
