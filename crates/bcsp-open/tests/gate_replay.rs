//! Replays the real 2026-08-19 partial-snapshot incident (captured from the
//! live Rutgers openSections feed, see `fixtures/README.md`) through the
//! snapshot integrity gate and asserts the design-mandated outcomes: every
//! partial sample is held, zero snapshots apply during the anomaly, and the
//! recovery snapshot applies immediately.

use std::collections::BTreeSet;

use bcsp_contracts::SectionIndex;
use bcsp_open::{
    CatalogSetIdentity, GATE_CONFIRM_MIN_SPAN_SECONDS, GateDecisionKind, GateDisposition,
    GateRuntime, GateSample, catalog_section_set_identity_v1,
};
use time::{Duration, OffsetDateTime};

/// Minimal parser for the fixture shape: a flat JSON array of 5-digit string
/// literals (`["00007","00010",...]`). Hand-rolled to keep the crate free of
/// a JSON dev-dependency; rejects anything outside that exact shape.
fn parse_index_array(raw: &str) -> BTreeSet<SectionIndex> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .expect("fixture must be a JSON array");
    inner
        .split(',')
        .map(|piece| {
            let piece = piece.trim();
            let literal = piece
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .expect("fixture entries must be string literals");
            SectionIndex::try_from(literal).expect("fixture entries must be valid indexes")
        })
        .collect()
}

fn fixture(name: &str) -> BTreeSet<SectionIndex> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    parse_index_array(&std::fs::read_to_string(path).expect("fixture readable"))
}

fn t0() -> OffsetDateTime {
    // 2026-08-19 21:17:16 UTC, the first partial sample's server time.
    OffsetDateTime::from_unix_timestamp(1_787_433_436).expect("incident epoch")
}

struct Replay {
    identity: CatalogSetIdentity,
    runtime: GateRuntime,
}

impl Replay {
    fn feed(&mut self, observed: &BTreeSet<SectionIndex>, at: OffsetDateTime) -> (GateDecisionKind, GateDisposition) {
        let sample = GateSample {
            catalog_identity: &self.identity,
            observed,
            observed_at: at,
        };
        let decision = self.runtime.evaluate(&sample);
        let outcome = (decision.kind, decision.disposition);
        self.runtime.advance(&decision);
        outcome
    }
}

fn incident_replay() -> Replay {
    let pre = fixture("pre_anomaly_full.json");
    assert_eq!(pre.len(), 11_423, "pre-anomaly fixture cardinality");
    // The catalog superset is not part of the capture; the intersection sets
    // themselves carry the gate-relevant information, so the identity is
    // derived from the pre-anomaly set and the baseline seeded from LKG.
    let identity = catalog_section_set_identity_v1(pre.iter());
    let runtime = GateRuntime::seeded(identity.clone(), pre.len() as u64);
    Replay { identity, runtime }
}

#[test]
fn replayed_incident_holds_every_partial_and_applies_recovery_immediately() {
    let partial = fixture("partial.json");
    let recovery = fixture("recovery_full.json");
    assert_eq!(partial.len(), 8_146, "partial fixture cardinality");
    assert_eq!(recovery.len(), 11_422, "recovery fixture cardinality");

    let mut replay = incident_replay();

    // Thirteen byte-identical partial samples over 41 seconds (the proven
    // incident timeline; ~3.5s cadence).
    let mut held = 0;
    for step in 0..13 {
        let at = t0() + Duration::milliseconds(step * 3_500);
        let (kind, disposition) = replay.feed(&partial, at);
        assert_eq!(kind, GateDecisionKind::Suspect, "partial sample {step} must be suspect");
        assert_eq!(disposition, GateDisposition::Hold, "partial sample {step} must be held");
        held += 1;
    }
    assert_eq!(held, 13);
    assert!(replay.runtime.is_quarantined(), "sticky quarantine across the anomaly");

    // Recovery snapshot 45 seconds after onset (21:18:01): applies at once.
    let (kind, disposition) = replay.feed(&recovery, t0() + Duration::seconds(45));
    assert_eq!(kind, GateDecisionKind::QuarantineRecover);
    assert_eq!(disposition, GateDisposition::Apply);
    assert!(!replay.runtime.is_quarantined());

    // Normal churn afterwards keeps passing.
    let (kind, _) = replay.feed(&recovery, t0() + Duration::seconds(75));
    assert_eq!(kind, GateDecisionKind::Pass);
}

#[test]
fn incident_shape_would_not_confirm_even_if_sustained_below_span() {
    // The incident lasted ~41s-105s; replay the identical partial right up to
    // (but not reaching) the confirmation span and assert it never applies.
    let partial = fixture("partial.json");
    let mut replay = incident_replay();

    let mut at = t0();
    let step = Duration::seconds(30);
    while (at - t0()).whole_seconds() < GATE_CONFIRM_MIN_SPAN_SECONDS {
        let (kind, disposition) = replay.feed(&partial, at);
        assert_eq!(kind, GateDecisionKind::Suspect);
        assert_eq!(disposition, GateDisposition::Hold);
        at += step;
    }
    // The next sample crosses the span with consistent content: this is the
    // deliberate availability/integrity trade-off boundary (design section
    // "Quarantined transitions"): a >=300s stable shrink is accepted.
    let (kind, disposition) = replay.feed(&partial, at);
    assert_eq!(kind, GateDecisionKind::QuarantineConfirm);
    assert_eq!(disposition, GateDisposition::Apply);
}

#[test]
fn v1_design_loophole_sequence_never_applies_the_drifting_partial() {
    // The reviewer's counterexample against per-sample thresholding:
    // 11423 -> 8146 (quarantined) -> 10300 (only 9.8% below baseline) must
    // NOT be applied while quarantined, because it is neither consistent
    // with the anchor nor back at the reference.
    let pre = fixture("pre_anomaly_full.json");
    let partial = fixture("partial.json");
    let mut replay = incident_replay();

    let (_, disposition) = replay.feed(&partial, t0());
    assert_eq!(disposition, GateDisposition::Hold);

    // Build a 10,300-strong drifting set: the partial plus a slice of the
    // missing sections (real indexes from the capture).
    let missing: Vec<&SectionIndex> = pre.difference(&partial).collect();
    let mut drifting = partial.clone();
    for index in missing.iter().take(10_300 - partial.len()) {
        drifting.insert((*index).clone());
    }
    assert_eq!(drifting.len(), 10_300);

    let (kind, disposition) = replay.feed(&drifting, t0() + Duration::seconds(10));
    assert_eq!(kind, GateDecisionKind::Suspect, "drifting partial re-anchors");
    assert_eq!(disposition, GateDisposition::Hold, "drifting partial must not apply");
    assert!(replay.runtime.is_quarantined());

    // Full recovery still applies immediately afterwards.
    let (kind, disposition) = replay.feed(&pre, t0() + Duration::seconds(20));
    assert_eq!(kind, GateDecisionKind::QuarantineRecover);
    assert_eq!(disposition, GateDisposition::Apply);
}
