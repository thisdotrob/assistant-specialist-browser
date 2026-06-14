//! Milestone 5 exit criterion: a CLI orchestrator delegates a simple browser
//! task to the browser specialist and receives a result.
//!
//! This drives the whole path without Docker or a real Chromium: the browser
//! profile is translated into the agent graph's plain `RegisteredProfile` data
//! (the one-way bridge), a specialist group is created under policy, a job runs
//! through the guarded state machine, a handoff packet is routed into the
//! specialist's session, the host approves a screenshot artifact against the
//! profile's artifact policy, a `FakeContainer` emits a structured result, the
//! result is collected and checked against the profile's artifact size ceiling,
//! the job completes, and the result is routed back to the orchestrator.

use std::path::PathBuf;

use assistant_agent_graph::{
    artifacts_within_policy, authorize_external_destination, collect_result, create_specialist,
    route_handoff, route_result, start_job, store, transition_job, validate_handoff, A2aAcl,
    AuditEvent, CredentialPolicy, HandoffPacket, JobBudget, JobEvent, JobStatus, MemoryFact,
    ResultArtifact, RetentionLabel, RoutingError, SpecialistJob, SpecialistRegistry,
    SpecialistResult, SpecialistStatus, VecAuditSink, RESULT_KIND,
};
use assistant_specialist_browser::{
    approve_artifact, ArtifactKind, BrowserSpecialistProfile, NetworkPolicy, BROWSER_PROFILE_ID,
};

use assistant_db::{apply, baseline_migrations, baseline_owner_modules};
use assistant_session::{LocalControl, SessionLayout};

use rusqlite::Connection;

fn migrated_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    let order: Vec<String> = baseline_owner_modules()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut set = baseline_migrations(order);
    for m in store::migrations() {
        set.add(m);
    }
    apply(&mut conn, &set).unwrap();
    conn
}

#[test]
fn orchestrator_delegates_browser_task_and_receives_result() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let conn = migrated_db();
    let mut audit = VecAuditSink::new();

    // The host owns the concrete browser profile; it browses only example.com.
    let profile = BrowserSpecialistProfile::new(NetworkPolicy::allowlist(["example.com"]));
    assert!(profile.network_policy().is_host_allowed("billing.example.com"));

    // Bridge the profile into the agent graph as plain registry data, then admit
    // it. The graph never links this crate; it only sees `RegisteredProfile`.
    let registered = profile.registered_profile();
    assert_eq!(registered.profile_id, BROWSER_PROFILE_ID);
    // A browser specialist may never own external channel destinations.
    assert!(matches!(
        authorize_external_destination(&registered),
        Err(RoutingError::ExternalDestinationForbidden { .. })
    ));
    let mut reg = SpecialistRegistry::new();
    reg.register(registered);

    // 1. Create the specialist group under the per-profile creation limit.
    let specialist_group =
        create_specialist(&conn, &reg, &mut audit, BROWSER_PROFILE_ID, "browser-1").unwrap();
    assert_eq!(specialist_group, "browser-1");

    // 2. Stand up the orchestrator and specialist sessions.
    let orch_layout = SessionLayout::derive(root, "orchestrator", "sess-1").unwrap();
    let spec_layout = SessionLayout::derive(root, "browser-1", "job-1").unwrap();
    let orch_control = LocalControl::new(orch_layout.clone());
    let spec_control = LocalControl::new(spec_layout.clone());
    orch_control.init().unwrap();
    spec_control.init().unwrap();

    // 3. Register the single allowed routing edge.
    let mut acl = A2aAcl::new();
    acl.allow("orchestrator", "browser-1");

    // 4. Start the job and drive it queued -> running.
    let job = SpecialistJob::new(
        "job-1",
        "orchestrator",
        "browser-1",
        BROWSER_PROFILE_ID,
        JobBudget {
            max_tokens: Some(2000),
            max_wall_secs: Some(120),
        },
        60,
    );
    start_job(&conn, &reg, &mut audit, &job).unwrap();
    assert_eq!(
        transition_job(&conn, &mut audit, "job-1", JobEvent::Start).unwrap(),
        JobStatus::Running
    );

    // 5. Route a handoff packet into the specialist's session.
    let mut handoff = HandoffPacket::new("Find the latest invoice total on the billing portal");
    handoff.facts.push(MemoryFact {
        text: "Account email is owner@example.com".into(),
        source: Some("memory:fact-42".into()),
        retention: RetentionLabel::CiteOnly,
    });
    handoff.constraints.push("do not place orders".into());
    handoff.credential_policy = CredentialPolicy {
        allowed_scopes: vec!["billing.read".into()],
        browser_session_allowed: true,
    };
    validate_handoff(&handoff).unwrap();
    let return_path = assistant_agent_graph::ReturnPath {
        orchestrator_group: "orchestrator".into(),
        session_id: "sess-1".into(),
        inbound_seq: 0,
    };
    route_handoff(
        &mut audit,
        &acl,
        &spec_layout,
        "orchestrator",
        "browser-1",
        &handoff,
        &return_path,
    )
    .unwrap();

    // 6. The fake browser container wakes, reads the handoff, and (host-side)
    //    the screenshot it wants to return is approved against the profile's
    //    artifact policy before it is emitted by reference.
    let spec_container = spec_control.fake_container();
    spec_container.start("run-1").unwrap();
    store::add_run_link(&conn, "job-1", "run-1").unwrap();
    let inbound = spec_container.read_inbound().unwrap();
    assert!(inbound.iter().any(|(_, c)| c.contains("invoice total")));

    let artifact_root = root.join("browser-1").join("artifacts");
    std::fs::create_dir_all(&artifact_root).unwrap();
    let shot_path = artifact_root.join("invoice.png");
    let shot_bytes: u64 = 2048;
    let policy = profile.artifact_policy(vec![artifact_root.clone()]);
    let approved = approve_artifact(&policy, ArtifactKind::Screenshot, shot_bytes, &shot_path)
        .expect("screenshot within policy and confined to an approved root");
    assert_eq!(approved, shot_path);
    // A capture aimed outside the approved roots is refused.
    assert!(approve_artifact(
        &policy,
        ArtifactKind::Screenshot,
        shot_bytes,
        &PathBuf::from("/etc/loose.png"),
    )
    .is_err());

    let mut result = SpecialistResult::new(SpecialistStatus::Completed, "found the invoice total");
    result.answer = "42".into();
    result.artifacts.push(ResultArtifact {
        artifact_id: "invoice-shot".into(),
        kind: ArtifactKind::Screenshot.as_str().into(),
        by_reference: true,
        size_bytes: Some(shot_bytes),
    });
    spec_container
        .emit(RESULT_KIND, &result.to_json().unwrap())
        .unwrap();

    // 7. Collect the result and enforce the profile's artifact ceiling.
    let collected = collect_result(&spec_layout).unwrap();
    assert_eq!(collected.status, SpecialistStatus::Completed);
    assert_eq!(collected.answer, "42");
    artifacts_within_policy(&collected, profile.max_artifact_bytes()).unwrap();

    // 8. Complete the job and route the result back to the orchestrator.
    assert_eq!(
        transition_job(&conn, &mut audit, "job-1", JobEvent::Complete).unwrap(),
        JobStatus::Succeeded
    );
    assert_eq!(
        store::load_job(&conn, "job-1").unwrap().status,
        JobStatus::Succeeded
    );
    route_result(&mut audit, &orch_layout, "browser-1", "orchestrator", &collected).unwrap();

    // 9. The orchestrator session now carries the specialist's result. Parse the
    //    delivered message back into a structured result rather than matching a
    //    substring, so the "receives a result" half of the exit criterion is
    //    asserted on the actual deliverable.
    let orch_container = orch_control.fake_container();
    let orch_inbound = orch_container.read_inbound().unwrap();
    let delivered = orch_inbound
        .iter()
        .find_map(|(_, c)| SpecialistResult::from_json(c).ok())
        .expect("orchestrator inbound carries a structured specialist result");
    assert_eq!(delivered.status, SpecialistStatus::Completed);
    assert_eq!(delivered.answer, "42");
    assert_eq!(delivered, collected);

    // 10. The audit trail captured creation, delegation, completion, and routing.
    assert!(audit.events.iter().any(|e| matches!(
        e,
        AuditEvent::SpecialistCreated { specialist_group, .. } if specialist_group == "browser-1"
    )));
    assert!(audit
        .events
        .iter()
        .any(|e| matches!(e, AuditEvent::DelegationStarted { job_id, .. } if job_id == "job-1")));
    assert!(audit.events.iter().any(|e| matches!(
        e,
        AuditEvent::DelegationCompleted { status: JobStatus::Succeeded, .. }
    )));
    assert!(audit.events.iter().any(|e| matches!(
        e,
        AuditEvent::AgentRouting { kind, .. } if kind == "specialist_handoff"
    )));
    assert!(audit.events.iter().any(|e| matches!(
        e,
        AuditEvent::AgentRouting { kind, .. } if kind == "specialist_result"
    )));
}
