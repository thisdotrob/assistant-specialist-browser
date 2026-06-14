# assistant-specialist-browser Contract

## Public API
`browser_specialist_spec(NetworkPolicy) -> SpecialistSpec`: the importable handle a product registers to gain web browsing. It bundles the browser specialist's routing identity (`route_name`/`description`), its agent-graph profile identity and concurrency limits, its custom image reference (`assistant-specialist-browser`), and the complete in-container turn config (the browser persona prompt, `tools: ["Bash"]`, `allowed_tools: ["Bash(agent-browser:*)"]`, `max_turns`). The crate also exports `network_guardrail`, the `BROWSER_*` identity consts, and the host-side policy types (`BrowserSpecialistProfile`, `ArtifactPolicy`, `NetworkPolicy`, readiness gates). It depends only on the plain `claw-specialist-spec` vocab crate, not on `claw-host` or core internals, so a product imports it as a self-contained unit. The co-located `image/Dockerfile` (`FROM claw-agent-base` + Chromium + `agent-browser`) builds the custom image the spec points at.

## Persistence Ownership
Owns browser-specialist capability and run-state metadata via its own extension tables (central DB migrations) where needed; does not mutate core, agent-graph, or capability base tables.

## Config
The `NetworkPolicy` (`{ mode: deny_all|allowlist|open, allowed_domains }`) is passed to `browser_specialist_spec` at registration time. For an `allowlist` policy the builder folds the permitted domains into the spec's `system_prompt` as a soft prompt-level guardrail (`network_guardrail`); `open` and `deny_all` add nothing. This is advisory only — the container has unrestricted network and Chromium reaches sites directly — not container-level enforcement. The guardrail is resolved host-side at spec-build time, so the container receives only the generic `CLAW_SPECIALIST_*` turn-config env; there is no browser-specific container env. The in-container executor is a real Claude turn that drives the `agent-browser` CLI (headless Chromium) to navigate and read pages.

## Events
Emits browser-session-started, artifact-captured, and browser-session-ended events.

## CLI/Web Surfaces
None directly; specialist runs are observed through agent-graph views in CLI and web.

## Prompt Fragments
Owns the browser specialist's persona prompt as data, authored here as a Rust const and carried in the spec's `system_prompt` (the generic shim harness uses it verbatim); the `network_guardrail` text is appended at spec-build time. None are platform-shared — this is the specialist's operational prompt, not a shared fragment.

## Readiness Checks
Verifies the custom image's binaries are present (Chromium + `agent-browser`), artifact storage is writable, and the network policy resolves.

## Conformance Tests
`browser_specialist_spec` carries the browser identity and image reference (route/profile/group/image/tools); an `allowlist` policy folds a domain guardrail into the spec's `system_prompt` while `open`/`deny_all`/empty-allowlist add none.
