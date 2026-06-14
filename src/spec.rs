//! The browser specialist's [`SpecialistSpec`] builder.
//!
//! This is the importable handle a product registers to gain web browsing: it
//! bundles the browser specialist's routing identity, its custom image
//! reference, and the complete in-container turn configuration (persona, tools,
//! limits). The host translates the plain spec into a runtime profile + image +
//! container env at registration time, so the host never depends on this crate.
//!
//! The persona prompt lives here (not in the shim) so the shim stays a generic,
//! specialist-agnostic harness: the host hands this prompt to the container as
//! `ASSISTANT_SPECIALIST_SYSTEM_PROMPT` and the harness uses it verbatim.

use assistant_specialist_spec::SpecialistSpec;

use crate::network::{NetworkMode, NetworkPolicy};
use crate::profile::{
    BROWSER_PROFILE_ID, BROWSER_PROFILE_VERSION, DEFAULT_MAX_ARTIFACT_BYTES,
    DEFAULT_MAX_CONCURRENT_JOBS, DEFAULT_MAX_SPECIALISTS,
};

/// The name the orchestrator routes by (the `delegate` tool's `specialist` enum
/// value). Also the suffix of the specialist's session group.
pub const BROWSER_ROUTE_NAME: &str = "browser";

/// The session-group slug the browser specialist's job containers live under.
pub const BROWSER_GROUP_SLUG: &str = "browser-1";

/// One-line capability description surfaced to the orchestrator for routing.
pub const BROWSER_DESCRIPTION: &str =
    "browses the web and reads live pages — for requests that need current web access";

/// The repository name of the browser specialist's custom image (built from the
/// co-located `image/Dockerfile`, `FROM assistant-base` + Chromium).
pub const BROWSER_IMAGE_REPOSITORY: &str = "assistant-specialist-browser";

/// Per-turn step ceiling, bounding a stuck or looping browse.
pub const BROWSER_MAX_TURNS: u32 = 40;

/// The browser specialist's persona. It drives `agent-browser` to gather facts
/// and reports them; its text is relayed to the end user by the orchestrator, so
/// it states findings plainly and never narrates its tooling.
const BROWSER_SYSTEM_PROMPT: &str = "You are a web browsing specialist. You have a real browser available through the `agent-browser` command, which you run with the Bash tool. Use it to open pages, read them, and interact (search, click, fill, follow links) as needed to satisfy the request.

Core workflow:
- `agent-browser open <url>` to navigate to a page.
- `agent-browser snapshot -i` to read the page as an accessibility tree of interactive elements tagged like [ref=e1]; act on them with `@e1` (e.g. `agent-browser click @e1`, `agent-browser fill @e2 \"text\"`, `agent-browser press Enter`).
- `agent-browser get text @e1` / `get title` / `get url` to read specific content; re-snapshot after the page changes.
- If no URL is given but a topic is, open a search engine (e.g. https://duckduckgo.com) and search.

When you have what the request needs, stop and write a clear, factual answer in plain prose: the specific information found, with the page title and source URL when relevant. Quote exact figures, names, or headings rather than paraphrasing loosely.

Important: your answer is relayed to the person who made the request, so write only the findings themselves. Do NOT describe how you obtained them — never mention the browser, the tool, commands, snapshots, refs, or that you are a specialist. If the page cannot be reached or the answer is not present, say so plainly and briefly.";

/// If the policy is an allowlist, render it as a soft domain guardrail appended
/// to the persona. `open` (unrestricted) and `deny_all` add nothing — the
/// guardrail is advisory prompt text, not a kernel sandbox.
pub fn network_guardrail(network: &NetworkPolicy) -> String {
    if network.mode == NetworkMode::Allowlist && !network.allowed_domains.is_empty() {
        format!(
            "\n\nOnly browse these domains (and their subdomains): {}. Do not navigate elsewhere.",
            network.allowed_domains.join(", ")
        )
    } else {
        String::new()
    }
}

/// Build the browser specialist's registration spec for the given egress policy.
/// The guardrail is folded into the system prompt at build time, so the host and
/// the generic shim harness need no browser-specific knowledge.
pub fn browser_specialist_spec(network: NetworkPolicy) -> SpecialistSpec {
    let system_prompt = format!("{BROWSER_SYSTEM_PROMPT}{}", network_guardrail(&network));
    SpecialistSpec {
        route_name: BROWSER_ROUTE_NAME.to_string(),
        description: BROWSER_DESCRIPTION.to_string(),
        profile_id: BROWSER_PROFILE_ID.to_string(),
        profile_version: BROWSER_PROFILE_VERSION.to_string(),
        group_slug: BROWSER_GROUP_SLUG.to_string(),
        image_repository: BROWSER_IMAGE_REPOSITORY.to_string(),
        image_tag: BROWSER_PROFILE_VERSION.to_string(),
        image_digest: None,
        max_specialists: DEFAULT_MAX_SPECIALISTS,
        max_concurrent_jobs: DEFAULT_MAX_CONCURRENT_JOBS,
        max_artifact_bytes: DEFAULT_MAX_ARTIFACT_BYTES,
        system_prompt,
        tools: vec!["Bash".to_string()],
        allowed_tools: vec!["Bash(agent-browser:*)".to_string()],
        max_turns: BROWSER_MAX_TURNS,
        extra_env: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_carries_browser_identity_and_image() {
        let spec = browser_specialist_spec(NetworkPolicy::open());
        assert_eq!(spec.route_name, BROWSER_ROUTE_NAME);
        assert_eq!(spec.profile_id, BROWSER_PROFILE_ID);
        assert_eq!(spec.profile_version, BROWSER_PROFILE_VERSION);
        assert_eq!(spec.group_slug, BROWSER_GROUP_SLUG);
        assert_eq!(spec.image_repository, BROWSER_IMAGE_REPOSITORY);
        assert_eq!(spec.image_tag, BROWSER_PROFILE_VERSION);
        assert_eq!(spec.image_digest, None);
        assert_eq!(spec.tools, vec!["Bash".to_string()]);
        assert_eq!(spec.allowed_tools, vec!["Bash(agent-browser:*)".to_string()]);
        assert_eq!(spec.max_turns, BROWSER_MAX_TURNS);
        assert_eq!(spec.max_specialists, DEFAULT_MAX_SPECIALISTS);
        assert_eq!(spec.max_concurrent_jobs, DEFAULT_MAX_CONCURRENT_JOBS);
        assert_eq!(spec.max_artifact_bytes, DEFAULT_MAX_ARTIFACT_BYTES);
    }

    #[test]
    fn open_and_deny_all_add_no_guardrail() {
        assert!(network_guardrail(&NetworkPolicy::open()).is_empty());
        assert!(network_guardrail(&NetworkPolicy::deny_all()).is_empty());
        let spec = browser_specialist_spec(NetworkPolicy::open());
        assert!(!spec.system_prompt.contains("Only browse these domains"));
        assert!(spec.system_prompt.contains("web browsing specialist"));
    }

    #[test]
    fn allowlist_folds_a_domain_guardrail_into_the_prompt() {
        let spec = browser_specialist_spec(NetworkPolicy::allowlist(["example.com", "docs.rs"]));
        assert!(spec.system_prompt.contains(
            "Only browse these domains (and their subdomains): example.com, docs.rs. Do not navigate elsewhere."
        ));
    }

    #[test]
    fn empty_allowlist_adds_no_guardrail() {
        // A misconfigured (empty) allowlist fails readiness elsewhere; here it
        // simply yields no guardrail text rather than an empty domain list.
        let spec = browser_specialist_spec(NetworkPolicy::allowlist(Vec::<String>::new()));
        assert!(!spec.system_prompt.contains("Only browse these domains"));
    }
}
