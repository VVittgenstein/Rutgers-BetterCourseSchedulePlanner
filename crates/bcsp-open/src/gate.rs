//! Snapshot integrity gate: per-target quarantine of implausibly shrunken
//! openSections snapshots.
//!
//! Pure decision core for the approved design in
//! `docs/design/2026-08-20-open-snapshot-integrity-gate.md` (v5). This module
//! holds no locks, reads no clock, and performs no IO: callers supply
//! observation time and route inputs by catalog identity, and the commit
//! protocol (per-target serial lock across decide -> transact -> advance)
//! lives in the wiring layer.
//!
//! Scope contract (design section 3): the gate evaluates only snapshots that
//! the existing pipeline would classify as applicable (`ValidApplied` /
//! `ValidEmptyNoRows`). Hard rejections (catalog race, zero intersection,
//! malformed or failed transport) never reach [`GateRuntime::evaluate`] and
//! never count as candidate samples.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;

use bcsp_contracts::SectionIndex;
use time::OffsetDateTime;

use crate::reconcile::canonical_open_set_hash_v1;

/// Relative shrink threshold: a snapshot missing at least 1/10 of the
/// baseline is a quarantine candidate (subject to [`GATE_MIN_ABSOLUTE_DROP`]).
pub const GATE_DROP_THRESHOLD_DENOMINATOR: u64 = 10;
/// Recovery hysteresis: leaving quarantine via recovery requires the
/// shortfall against the frozen reference to close to under 1/20 (half the
/// entry threshold). Without this, a drifting partial 9.x% below the
/// reference would be misjudged as recovered and applied -- the reviewer's
/// original v1 counterexample (11423 -> 8146 -> 10300 -> 11422). Samples in
/// the 5-10% band stay held until they either recover fully or stabilize
/// into a confirmation.
pub const GATE_RECOVERY_THRESHOLD_DENOMINATOR: u64 = 20;
/// Absolute floor: shrinkage below this many sections never quarantines,
/// which keeps legitimately tiny targets (for example between-session
/// campuses) on the pre-gate behavior.
pub const GATE_MIN_ABSOLUTE_DROP: u64 = 25;
/// Rolling window of applied intersection counts kept per baseline epoch.
pub const GATE_BASELINE_WINDOW: usize = 16;
/// Minimum count of mutually consistent quarantine samples before the gate
/// may confirm a shrunken snapshot as the new reality.
pub const GATE_CONFIRM_MIN_CONSISTENT: u32 = 3;
/// Minimum span between the first and latest consistent sample before the
/// gate may confirm ("no earlier than" semantics).
pub const GATE_CONFIRM_MIN_SPAN_SECONDS: i64 = 300;
/// Maximum spacing between adjacent candidate samples; a larger gap voids the
/// candidate episode (re-anchor) so slow trickles never stitch a confirmation.
pub const GATE_MAX_SAMPLE_GAP_SECONDS: i64 = 120;
/// Symmetric-difference tolerance (ceil of this fraction of the anchor size)
/// for treating a sample as consistent with the candidate anchor.
pub const GATE_CONSISTENCY_DENOMINATOR: usize = 100;
/// Upper bound for the dedicated quarantine probe cadence; the effective
/// cadence is `min(this, normal watch interval)` for every campus.
pub const GATE_QUARANTINE_PROBE_CAP_SECONDS: u64 = 30;

const CATALOG_IDENTITY_PREIMAGE: &str = "bcsp-catalog-set-v1\n";

/// Semantic identity of a catalog section set. Two catalogs with the same
/// section indexes share an identity regardless of content version numbers
/// (unpublished candidates may reuse `serving + 1`, so version numbers cannot
/// identify a catalog).
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CatalogSetIdentity(String);

impl CatalogSetIdentity {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Computes the semantic identity of a catalog section set.
pub fn catalog_section_set_identity_v1<'a>(
    indexes: impl IntoIterator<Item = &'a SectionIndex>,
) -> CatalogSetIdentity {
    let mut preimage = String::from(CATALOG_IDENTITY_PREIMAGE);
    for index in indexes {
        preimage.push_str(index.as_str());
        preimage.push('\n');
    }
    CatalogSetIdentity(bcsp_rutgers_client::sha256_hex(preimage.as_bytes()))
}

/// One applicable snapshot presented to the gate. `observed` is the
/// intersection of the response set with the current catalog (orphans are
/// excluded upstream), matching the count that drives every threshold.
#[derive(Clone, Debug)]
pub struct GateSample<'a> {
    pub catalog_identity: &'a CatalogSetIdentity,
    pub observed: &'a BTreeSet<SectionIndex>,
    pub observed_at: OffsetDateTime,
}

/// Whether the snapshot may be applied to durable state and fanned out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateDisposition {
    Apply,
    Hold,
}

/// Which rule produced the decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateDecisionKind {
    /// Healthy and unremarkable (includes the no-baseline case).
    Pass,
    /// Entering or remaining in quarantine; the snapshot is withheld.
    Suspect,
    /// A quarantined target saw the feed return to the frozen reference.
    QuarantineRecover,
    /// A shrunken feed proved stable long enough to accept as the new
    /// reality; the baseline is rebased on it.
    QuarantineConfirm,
}

/// The gate's verdict for one sample, bound to the inputs that produced it.
/// Produced under the per-target serial lock and applied via
/// [`GateRuntime::advance`] only after the surrounding transaction commits.
#[derive(Clone, Debug)]
pub struct GateDecision {
    pub disposition: GateDisposition,
    pub kind: GateDecisionKind,
    pub catalog_identity: CatalogSetIdentity,
    pub observed_intersection_count: u64,
    pub response_set_sha256: String,
    pub gate_epoch: u64,
    next_state: GateState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaselineEpoch {
    catalog_identity: CatalogSetIdentity,
    window: VecDeque<u64>,
    /// Floor supplied by the wiring layer from the persisted last-known-good
    /// intersection count (survives restarts; `None` for in-memory-only).
    lkg_floor: Option<u64>,
}

impl BaselineEpoch {
    fn seeded(catalog_identity: CatalogSetIdentity, lkg_floor: Option<u64>) -> Self {
        Self {
            catalog_identity,
            window: VecDeque::new(),
            lkg_floor,
        }
    }

    fn push_applied(&mut self, observed: u64) {
        if self.window.len() == GATE_BASELINE_WINDOW {
            self.window.pop_front();
        }
        self.window.push_back(observed);
    }

    fn lower_median(&self) -> Option<u64> {
        if self.window.is_empty() {
            return None;
        }
        let mut sorted: Vec<u64> = self.window.iter().copied().collect();
        sorted.sort_unstable();
        Some(sorted[(sorted.len() - 1) / 2])
    }

    /// `max(lower_median(window), lkg_floor)`, or `None` when neither exists.
    pub fn value(&self) -> Option<u64> {
        match (self.lower_median(), self.lkg_floor) {
            (Some(median), Some(floor)) => Some(median.max(floor)),
            (Some(median), None) => Some(median),
            (None, Some(floor)) => Some(floor),
            (None, None) => None,
        }
    }

    pub fn set_lkg_floor(&mut self, floor: Option<u64>) {
        self.lkg_floor = floor;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateEpisode {
    /// `None` after a restart rebuild: the set body is not persisted, so the
    /// first post-restart sample continues the episode only on exact hash
    /// equality (design residual risk 3).
    anchor_set: Option<BTreeSet<SectionIndex>>,
    anchor_set_sha256: String,
    first_seen: OffsetDateTime,
    last_seen: OffsetDateTime,
    consistent_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GateState {
    Healthy {
        baseline: Option<BaselineEpoch>,
    },
    Quarantined {
        /// Baseline frozen at quarantine entry; its [`BaselineEpoch::value`]
        /// is the recovery reference and its window is preserved so recovery
        /// resumes the pre-quarantine history.
        baseline: BaselineEpoch,
        episode: CandidateEpisode,
    },
}

/// Per-(target, catalog-identity) gate state machine plus its monotonically
/// increasing epoch. The epoch exists across both states and increments on
/// every applied transition (including window growth), serving as the
/// serial-lock invariant assertion required by design section 6.
#[derive(Clone, Debug)]
pub struct GateRuntime {
    epoch: u64,
    state: GateState,
}

impl GateRuntime {
    /// A runtime with no baseline: every sample passes until the wiring seeds
    /// one (first applied snapshot, or an LKG transfer).
    pub fn new_unseeded() -> Self {
        Self {
            epoch: 0,
            state: GateState::Healthy { baseline: None },
        }
    }

    /// A runtime seeded from a persisted or transferred intersection count
    /// (`serving LKG` on restart, `|serving LKG ∩ candidate catalog|` for a
    /// candidate runtime).
    pub fn seeded(catalog_identity: CatalogSetIdentity, lkg_floor: u64) -> Self {
        Self {
            epoch: 0,
            state: GateState::Healthy {
                baseline: Some(BaselineEpoch::seeded(catalog_identity, Some(lkg_floor))),
            },
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn state(&self) -> &GateState {
        &self.state
    }

    pub fn is_quarantined(&self) -> bool {
        matches!(self.state, GateState::Quarantined { .. })
    }

    /// Suspect entry rule: `missing >= max(ceil(baseline / 10), 25)`.
    /// One-sided (growth never quarantines); `baseline == 0` never trips
    /// because the absolute floor exceeds any possible shortfall.
    fn shrink_is_suspect(baseline: u64, observed: u64) -> bool {
        let missing = baseline.saturating_sub(observed);
        let relative = baseline.div_ceil(GATE_DROP_THRESHOLD_DENOMINATOR);
        missing >= relative.max(GATE_MIN_ABSOLUTE_DROP)
    }

    /// Recovery exit rule (hysteresis): the shortfall against the frozen
    /// reference must close to under `max(ceil(reference / 20), 25)` --
    /// strictly tighter than the entry rule, so a sample the entry rule
    /// would merely not flag cannot exit an active quarantine.
    fn recovered_near_reference(reference: u64, observed: u64) -> bool {
        let missing = reference.saturating_sub(observed);
        let relative = reference.div_ceil(GATE_RECOVERY_THRESHOLD_DENOMINATOR);
        missing < relative.max(GATE_MIN_ABSOLUTE_DROP)
    }

    fn consistent_with_anchor(episode: &CandidateEpisode, sample: &GateSample<'_>, sample_hash: &str) -> bool {
        match &episode.anchor_set {
            Some(anchor) => {
                let tolerance = anchor.len().div_ceil(GATE_CONSISTENCY_DENOMINATOR);
                let symdiff = anchor.symmetric_difference(sample.observed).count();
                symdiff <= tolerance
            }
            // Post-restart episode: only exact byte identity continues it.
            None => episode.anchor_set_sha256 == sample_hash,
        }
    }

    fn anchored_episode(sample: &GateSample<'_>, sample_hash: String) -> CandidateEpisode {
        CandidateEpisode {
            anchor_set: Some(sample.observed.clone()),
            anchor_set_sha256: sample_hash,
            first_seen: sample.observed_at,
            last_seen: sample.observed_at,
            consistent_count: 1,
        }
    }

    /// Evaluates one applicable sample without mutating state. The caller
    /// holds the per-target serial lock across `evaluate`, the storage
    /// transaction, and [`GateRuntime::advance`].
    ///
    /// # Panics
    ///
    /// Panics if the sample's catalog identity does not match the identity
    /// this runtime is keyed by while a baseline exists: routing by identity
    /// is the [`TargetGateSet`]'s contract and a mismatch is a wiring defect.
    pub fn evaluate(&self, sample: &GateSample<'_>) -> GateDecision {
        let observed_count = sample.observed.len() as u64;
        let sample_hash = canonical_open_set_hash_v1(sample.observed.iter());
        let decision = |kind: GateDecisionKind, disposition: GateDisposition, next_state: GateState| GateDecision {
            disposition,
            kind,
            catalog_identity: sample.catalog_identity.clone(),
            observed_intersection_count: observed_count,
            response_set_sha256: sample_hash.clone(),
            gate_epoch: self.epoch,
            next_state,
        };

        match &self.state {
            GateState::Healthy { baseline: None } => {
                // No baseline: pass, and let the wiring decide when to seed.
                decision(
                    GateDecisionKind::Pass,
                    GateDisposition::Apply,
                    GateState::Healthy { baseline: None },
                )
            }
            GateState::Healthy {
                baseline: Some(baseline),
            } => {
                assert_eq!(
                    &baseline.catalog_identity, sample.catalog_identity,
                    "gate sample routed to a runtime keyed by a different catalog identity",
                );
                let reference = baseline.value().unwrap_or(0);
                if Self::shrink_is_suspect(reference, observed_count) {
                    let episode = Self::anchored_episode(sample, sample_hash.clone());
                    decision(
                        GateDecisionKind::Suspect,
                        GateDisposition::Hold,
                        GateState::Quarantined {
                            baseline: baseline.clone(),
                            episode,
                        },
                    )
                } else {
                    let mut advanced = baseline.clone();
                    advanced.push_applied(observed_count);
                    decision(
                        GateDecisionKind::Pass,
                        GateDisposition::Apply,
                        GateState::Healthy {
                            baseline: Some(advanced),
                        },
                    )
                }
            }
            GateState::Quarantined { baseline, episode } => {
                assert_eq!(
                    &baseline.catalog_identity, sample.catalog_identity,
                    "gate sample routed to a runtime keyed by a different catalog identity",
                );
                let reference = baseline.value().unwrap_or(0);
                if Self::recovered_near_reference(reference, observed_count) {
                    // Recovery: the feed is back near the frozen reference.
                    // The pre-quarantine window is preserved and advanced.
                    let mut recovered = baseline.clone();
                    recovered.push_applied(observed_count);
                    return decision(
                        GateDecisionKind::QuarantineRecover,
                        GateDisposition::Apply,
                        GateState::Healthy {
                            baseline: Some(recovered),
                        },
                    );
                }
                let gap_seconds = (sample.observed_at - episode.last_seen).whole_seconds();
                if gap_seconds > GATE_MAX_SAMPLE_GAP_SECONDS
                    || !Self::consistent_with_anchor(episode, sample, &sample_hash)
                {
                    // Over-gap or divergent content: void the candidate and
                    // re-anchor on this sample. Spans never stitch across
                    // differing sets or long silences.
                    let episode = Self::anchored_episode(sample, sample_hash.clone());
                    return decision(
                        GateDecisionKind::Suspect,
                        GateDisposition::Hold,
                        GateState::Quarantined {
                            baseline: baseline.clone(),
                            episode,
                        },
                    );
                }
                let mut continued = episode.clone();
                continued.consistent_count = continued.consistent_count.saturating_add(1);
                continued.last_seen = sample.observed_at;
                if continued.anchor_set.is_none() {
                    // First post-restart continuation restores the set body.
                    continued.anchor_set = Some(sample.observed.clone());
                }
                let span_seconds = (continued.last_seen - continued.first_seen).whole_seconds();
                if continued.consistent_count >= GATE_CONFIRM_MIN_CONSISTENT
                    && span_seconds >= GATE_CONFIRM_MIN_SPAN_SECONDS
                {
                    // Confirmed new reality: apply and rebase the baseline on
                    // the confirmed cardinality (fresh window, no stale floor).
                    let mut rebased =
                        BaselineEpoch::seeded(baseline.catalog_identity.clone(), None);
                    rebased.push_applied(observed_count);
                    return decision(
                        GateDecisionKind::QuarantineConfirm,
                        GateDisposition::Apply,
                        GateState::Healthy {
                            baseline: Some(rebased),
                        },
                    );
                }
                decision(
                    GateDecisionKind::Suspect,
                    GateDisposition::Hold,
                    GateState::Quarantined {
                        baseline: baseline.clone(),
                        episode: continued,
                    },
                )
            }
        }
    }

    /// Applies a decision after the surrounding transaction committed.
    ///
    /// # Panics
    ///
    /// Panics when the decision was computed against a different epoch. Under
    /// the per-target serial lock this is impossible; a panic means the lock
    /// was bypassed and the process must not continue silently (design
    /// section 6: the epoch is an invariant assertion, not a control path).
    pub fn advance(&mut self, decision: &GateDecision) {
        assert_eq!(
            decision.gate_epoch, self.epoch,
            "gate decision applied against a stale epoch: the per-target serial lock was bypassed",
        );
        self.state = decision.next_state.clone();
        self.epoch += 1;
    }

    /// Seeds or replaces the baseline on a catalog identity change while
    /// healthy (design section 4, Healthy transitions): the window resets and
    /// the transferred `|LKG open set ∩ new catalog|` count becomes the floor.
    /// `None` suspends the gate until the first applied snapshot reseeds it.
    pub fn reset_for_catalog_identity(
        &mut self,
        catalog_identity: CatalogSetIdentity,
        transferred_floor: Option<u64>,
    ) {
        self.state = GateState::Healthy {
            baseline: transferred_floor
                .map(|floor| BaselineEpoch::seeded(catalog_identity, Some(floor))),
        };
        self.epoch += 1;
    }
}

/// Newest-first summary of one persisted attempt, as needed by the restart
/// rebuild rules (design section 4, restart).
#[derive(Clone, Debug)]
pub struct RestartAttemptSummary {
    pub is_suspect_partial: bool,
    pub canonical_set_sha256: String,
    pub completed_at: OffsetDateTime,
}

/// Rebuilds a serving runtime after process restart.
///
/// Re-enters quarantine only when the newest attempt is a suspect and the
/// suspect run is contiguous (any other classification in between resets to
/// healthy), the run shares one canonical hash, an LKG-derived reference
/// exists, and the newest suspect is no older than [`GATE_MAX_SAMPLE_GAP_SECONDS`]
/// (cross-restart gap rule). The rebuilt episode carries the persisted hash
/// but no set body; continuation therefore requires exact hash equality.
pub fn rebuild_after_restart(
    catalog_identity: CatalogSetIdentity,
    lkg_reference: Option<u64>,
    attempts_newest_first: &[RestartAttemptSummary],
    now: OffsetDateTime,
) -> GateRuntime {
    let healthy = |floor: Option<u64>| match floor {
        Some(floor) => GateRuntime::seeded(catalog_identity.clone(), floor),
        None => GateRuntime::new_unseeded(),
    };
    let Some(newest) = attempts_newest_first.first() else {
        return healthy(lkg_reference);
    };
    if !newest.is_suspect_partial {
        return healthy(lkg_reference);
    }
    let Some(reference) = lkg_reference else {
        return healthy(None);
    };
    if (now - newest.completed_at).whole_seconds() > GATE_MAX_SAMPLE_GAP_SECONDS {
        return healthy(Some(reference));
    }
    let mut run: Vec<&RestartAttemptSummary> = Vec::new();
    for attempt in attempts_newest_first {
        if attempt.is_suspect_partial && attempt.canonical_set_sha256 == newest.canonical_set_sha256
        {
            run.push(attempt);
        } else {
            break;
        }
    }
    let oldest = run.last().expect("run contains at least the newest attempt");
    let mut baseline = BaselineEpoch::seeded(catalog_identity, Some(reference));
    baseline.set_lkg_floor(Some(reference));
    GateRuntime {
        epoch: 0,
        state: GateState::Quarantined {
            baseline,
            episode: CandidateEpisode {
                anchor_set: None,
                anchor_set_sha256: newest.canonical_set_sha256.clone(),
                first_seen: oldest.completed_at,
                last_seen: newest.completed_at,
                consistent_count: run.len() as u32,
            },
        },
    }
}

/// The serving runtime plus independent candidate runtimes keyed by catalog
/// identity (design section 4, serving/candidate dual runtime). Candidate
/// commits with a different catalog identity never read or write the serving
/// state; a successful candidate publish atomically promotes its runtime.
#[derive(Debug, Default)]
pub struct TargetGateSet {
    serving: Option<GateRuntime>,
    candidates: BTreeMap<CatalogSetIdentity, GateRuntime>,
}

impl TargetGateSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn serving_mut(&mut self) -> &mut GateRuntime {
        self.serving.get_or_insert_with(GateRuntime::new_unseeded)
    }

    pub fn install_serving(&mut self, runtime: GateRuntime) {
        self.serving = Some(runtime);
    }

    /// Returns the candidate runtime for `identity`, creating it with the
    /// supplied seed (`|serving LKG ∩ candidate catalog|`) on first use.
    pub fn candidate_mut(
        &mut self,
        identity: &CatalogSetIdentity,
        seed: impl FnOnce() -> Option<u64>,
    ) -> &mut GateRuntime {
        self.candidates
            .entry(identity.clone())
            .or_insert_with(|| match seed() {
                Some(floor) => GateRuntime::seeded(identity.clone(), floor),
                None => GateRuntime::new_unseeded(),
            })
    }

    /// Promotes a successfully published candidate to serving and clears the
    /// candidate table. Call inside the same serial-lock critical section as
    /// the publish transaction.
    pub fn promote_candidate(&mut self, identity: &CatalogSetIdentity) {
        if let Some(promoted) = self.candidates.remove(identity) {
            self.serving = Some(promoted);
        }
        self.candidates.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn index(value: u32) -> SectionIndex {
        SectionIndex::try_from(format!("{value:05}").as_str()).expect("test index")
    }

    fn set(range: std::ops::Range<u32>) -> BTreeSet<SectionIndex> {
        range.map(index).collect()
    }

    fn identity_for(set: &BTreeSet<SectionIndex>) -> CatalogSetIdentity {
        catalog_section_set_identity_v1(set.iter())
    }

    fn t0() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_770_000_000).expect("test epoch")
    }

    fn sample<'a>(
        identity: &'a CatalogSetIdentity,
        observed: &'a BTreeSet<SectionIndex>,
        at: OffsetDateTime,
    ) -> GateSample<'a> {
        GateSample {
            catalog_identity: identity,
            observed,
            observed_at: at,
        }
    }

    fn apply(runtime: &mut GateRuntime, s: &GateSample<'_>) -> GateDecision {
        let decision = runtime.evaluate(s);
        runtime.advance(&decision);
        decision
    }

    #[test]
    fn no_baseline_passes_and_zero_baseline_never_trips() {
        let full = set(0..100);
        let identity = identity_for(&full);
        let mut unseeded = GateRuntime::new_unseeded();
        let d = apply(&mut unseeded, &sample(&identity, &full, t0()));
        assert_eq!(d.kind, GateDecisionKind::Pass);
        assert_eq!(d.disposition, GateDisposition::Apply);

        let empty = BTreeSet::new();
        let mut zero = GateRuntime::seeded(identity.clone(), 0);
        let d = apply(&mut zero, &sample(&identity, &empty, t0()));
        assert_eq!(d.kind, GateDecisionKind::Pass);
    }

    #[test]
    fn small_targets_and_growth_never_quarantine() {
        let identity = identity_for(&set(0..50));
        // 49 -> 0: missing 49, but threshold = max(ceil(49/10)=5, 25) = 25;
        // 49 >= 25 would trip -- verify the dual rule blocks only < 25.
        let mut runtime = GateRuntime::seeded(identity.clone(), 24);
        let empty = BTreeSet::new();
        let d = apply(&mut runtime, &sample(&identity, &empty, t0()));
        assert_eq!(d.kind, GateDecisionKind::Pass, "24 -> 0 stays under the absolute floor");

        // Growth is never suspect.
        let grown = set(0..500);
        let mut runtime = GateRuntime::seeded(identity.clone(), 100);
        let d = apply(&mut runtime, &sample(&identity, &grown, t0()));
        assert_eq!(d.kind, GateDecisionKind::Pass);
    }

    #[test]
    fn large_shrink_enters_sticky_quarantine_and_recovery_applies() {
        let full = set(0..1000);
        let partial = set(0..700);
        let identity = identity_for(&full);
        let mut runtime = GateRuntime::seeded(identity.clone(), 1000);

        let d = apply(&mut runtime, &sample(&identity, &partial, t0()));
        assert_eq!(d.kind, GateDecisionKind::Suspect);
        assert_eq!(d.disposition, GateDisposition::Hold);
        assert!(runtime.is_quarantined());

        // Recovery hysteresis: a drifting sample in the 5-10% band (920 of
        // 1000 = 8% short) clears the ENTRY threshold but not the tighter
        // recovery threshold -- it must stay held and re-anchor, never apply.
        let drifting = set(0..920);
        let d = apply(&mut runtime, &sample(&identity, &drifting, t0() + Duration::seconds(10)));
        assert_eq!(d.kind, GateDecisionKind::Suspect, "8% short: inside hysteresis band, held");
        assert_eq!(d.disposition, GateDisposition::Hold);
        assert!(runtime.is_quarantined());

        let recovered = set(0..960);
        let d = apply(&mut runtime, &sample(&identity, &recovered, t0() + Duration::seconds(20)));
        assert_eq!(d.kind, GateDecisionKind::QuarantineRecover);
        assert_eq!(d.disposition, GateDisposition::Apply);
        assert!(!runtime.is_quarantined());
    }

    #[test]
    fn confirmation_requires_count_span_and_bounded_gaps() {
        let full = set(0..1000);
        let partial = set(0..600);
        let identity = identity_for(&full);
        let mut runtime = GateRuntime::seeded(identity.clone(), 1000);

        apply(&mut runtime, &sample(&identity, &partial, t0()));
        // Second consistent sample 100s later: count=2, span=100 -> still held.
        let d = apply(&mut runtime, &sample(&identity, &partial, t0() + Duration::seconds(100)));
        assert_eq!(d.kind, GateDecisionKind::Suspect);
        // Third consistent sample at 200s: count=3 but span 200 < 300 -> held.
        let d = apply(&mut runtime, &sample(&identity, &partial, t0() + Duration::seconds(200)));
        assert_eq!(d.kind, GateDecisionKind::Suspect);
        // Fourth at 310s: count=4, span=310 >= 300, gaps all <= 120 -> confirm.
        let d = apply(&mut runtime, &sample(&identity, &partial, t0() + Duration::seconds(310)));
        assert_eq!(d.kind, GateDecisionKind::QuarantineConfirm);
        assert_eq!(d.disposition, GateDisposition::Apply);
        assert!(!runtime.is_quarantined());

        // After confirmation the shrunken cardinality is the new baseline:
        // the same feed keeps applying.
        let d = apply(&mut runtime, &sample(&identity, &partial, t0() + Duration::seconds(340)));
        assert_eq!(d.kind, GateDecisionKind::Pass);
    }

    #[test]
    fn over_gap_and_divergent_sets_reanchor_without_span_stitching() {
        let full = set(0..1000);
        let partial_a = set(0..600);
        let partial_b = set(200..800);
        let identity = identity_for(&full);
        let mut runtime = GateRuntime::seeded(identity.clone(), 1000);

        apply(&mut runtime, &sample(&identity, &partial_a, t0()));
        // Divergent shrunken set: re-anchor (first_seen resets to this sample).
        apply(&mut runtime, &sample(&identity, &partial_b, t0() + Duration::seconds(30)));
        // Consistent with the new anchor but after an over-gap: re-anchor again.
        apply(
            &mut runtime,
            &sample(&identity, &partial_b, t0() + Duration::seconds(30 + 121)),
        );
        // Three more consistent samples within bounds, but the span must be
        // measured from the LAST re-anchor -- confirmation only at >= 300s
        // after it, proving no stitching happened.
        let base = t0() + Duration::seconds(151);
        let d = apply(&mut runtime, &sample(&identity, &partial_b, base + Duration::seconds(100)));
        assert_eq!(d.kind, GateDecisionKind::Suspect);
        let d = apply(&mut runtime, &sample(&identity, &partial_b, base + Duration::seconds(200)));
        assert_eq!(d.kind, GateDecisionKind::Suspect);
        let d = apply(&mut runtime, &sample(&identity, &partial_b, base + Duration::seconds(310)));
        assert_eq!(d.kind, GateDecisionKind::QuarantineConfirm);
    }

    #[test]
    fn advance_with_stale_epoch_panics() {
        let full = set(0..100);
        let identity = identity_for(&full);
        let mut runtime = GateRuntime::new_unseeded();
        let stale = runtime.evaluate(&sample(&identity, &full, t0()));
        runtime.advance(&stale);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.advance(&stale);
        }));
        assert!(result.is_err(), "second advance with the same decision must panic");
    }

    #[test]
    fn catalog_identity_reset_reseeds_or_suspends() {
        let old_set = set(0..1000);
        let new_set = set(100..1100);
        let old_identity = identity_for(&old_set);
        let new_identity = identity_for(&new_set);
        let mut runtime = GateRuntime::seeded(old_identity, 1000);

        runtime.reset_for_catalog_identity(new_identity.clone(), Some(900));
        let partial = set(100..700);
        let d = runtime.evaluate(&sample(&new_identity, &partial, t0()));
        assert_eq!(d.kind, GateDecisionKind::Suspect, "transferred floor keeps guarding");

        runtime.reset_for_catalog_identity(new_identity.clone(), None);
        let d = runtime.evaluate(&sample(&new_identity, &partial, t0()));
        assert_eq!(d.kind, GateDecisionKind::Pass, "no transfer -> gate suspended until reseeded");
    }

    #[test]
    fn restart_rebuild_continues_only_contiguous_same_hash_recent_runs() {
        let full = set(0..1000);
        let partial = set(0..600);
        let identity = identity_for(&full);
        let hash = canonical_open_set_hash_v1(partial.iter());
        let attempt = |suspect: bool, hash: &str, age: i64| RestartAttemptSummary {
            is_suspect_partial: suspect,
            canonical_set_sha256: hash.to_string(),
            completed_at: t0() - Duration::seconds(age),
        };

        // Contiguous same-hash suspect run, newest 60s old -> quarantined,
        // count/span inherited from the persisted run.
        let rebuilt = rebuild_after_restart(
            identity.clone(),
            Some(1000),
            &[attempt(true, &hash, 60), attempt(true, &hash, 90), attempt(false, "x", 120)],
            t0(),
        );
        assert!(rebuilt.is_quarantined());
        // Exact-hash sample continues the episode across the restart.
        let mut rebuilt = rebuilt;
        let d = apply(&mut rebuilt, &sample(&identity, &partial, t0() + Duration::seconds(10)));
        assert_eq!(d.kind, GateDecisionKind::Suspect);
        match rebuilt.state() {
            GateState::Quarantined { episode, .. } => {
                assert_eq!(episode.consistent_count, 3, "2 persisted + 1 live");
                assert_eq!(episode.first_seen, t0() - Duration::seconds(90));
            }
            GateState::Healthy { .. } => panic!("must remain quarantined"),
        }

        // Newest suspect too old -> healthy with the LKG floor.
        let rebuilt = rebuild_after_restart(
            identity.clone(),
            Some(1000),
            &[attempt(true, &hash, 121)],
            t0(),
        );
        assert!(!rebuilt.is_quarantined());

        // Interleaved failure caps the run at the newest suspect: still
        // quarantined (the fresh suspect is real evidence), but the episode
        // does not inherit anything from before the interruption.
        let rebuilt = rebuild_after_restart(
            identity.clone(),
            Some(1000),
            &[attempt(true, &hash, 30), attempt(false, "x", 60), attempt(true, &hash, 90)],
            t0(),
        );
        match rebuilt.state() {
            GateState::Quarantined { episode, .. } => {
                assert_eq!(episode.consistent_count, 1, "run stops at the interruption");
                assert_eq!(episode.first_seen, t0() - Duration::seconds(30));
            }
            GateState::Healthy { .. } => panic!("fresh suspect evidence must re-enter quarantine"),
        }

        // No LKG reference -> cannot quarantine.
        let rebuilt =
            rebuild_after_restart(identity.clone(), None, &[attempt(true, &hash, 30)], t0());
        assert!(!rebuilt.is_quarantined());
    }

    #[test]
    fn candidate_runtime_is_isolated_and_promotion_replaces_serving() {
        let serving_catalog = set(0..1000);
        let candidate_catalog = set(50..1050);
        let serving_identity = identity_for(&serving_catalog);
        let candidate_identity = identity_for(&candidate_catalog);

        let mut gates = TargetGateSet::new();
        gates.install_serving(GateRuntime::seeded(serving_identity, 1000));

        // Candidate first pull pairs with a partial feed: with the
        // |serving LKG ∩ candidate| seed it must Hold (the v5 bypass test).
        let candidate_partial = set(50..750);
        let candidate = gates.candidate_mut(&candidate_identity, || Some(950));
        let d = candidate.evaluate(&sample(&candidate_identity, &candidate_partial, t0()));
        assert_eq!(d.disposition, GateDisposition::Hold);
        candidate.advance(&d);
        assert!(candidate.is_quarantined());
        assert!(!gates.serving_mut().is_quarantined(), "serving state untouched");

        // Candidate retries with a full feed -> recover -> publish -> promote.
        let candidate_full = set(50..1040);
        let candidate = gates.candidate_mut(&candidate_identity, || Some(950));
        let d = candidate.evaluate(&sample(&candidate_identity, &candidate_full, t0() + Duration::seconds(30)));
        assert_eq!(d.disposition, GateDisposition::Apply);
        candidate.advance(&d);
        gates.promote_candidate(&candidate_identity);
        assert!(!gates.serving_mut().is_quarantined());
    }
}
