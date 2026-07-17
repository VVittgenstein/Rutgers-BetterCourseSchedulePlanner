use bcsp_contracts::{CampusCode, ServiceTermPublicationV2, TermCampusKey, TermId};
use bcsp_operational_storage::PublishedDiscoverySnapshot;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::refresh_generation::DISCOVERY_REFRESH_INTERVAL;

/// The only Rutgers Campus identities exposed by the Round 4 product surface.
///
/// Online delivery remains a course/section attribute. `ONLINE_*` selector aliases and every
/// other Rutgers selector Campus stay in raw discovery evidence but never become product targets.
pub const PRODUCT_CAMPUS_CODES: [&str; 3] = ["NB", "NK", "CM"];

pub fn is_product_campus(campus: &str) -> bool {
    PRODUCT_CAMPUS_CODES
        .iter()
        .any(|supported| campus.eq_ignore_ascii_case(supported))
}

pub fn product_target_keys(term: &TermId) -> Vec<TermCampusKey> {
    PRODUCT_CAMPUS_CODES
        .iter()
        .map(|campus| {
            TermCampusKey::new(
                term.clone(),
                CampusCode::try_from(*campus)
                    .expect("the frozen product Campus codes satisfy the wire contract"),
            )
        })
        .collect()
}

/// Projects the Rutgers publication evidence for one product-window term.
///
/// The last successful discovery remains authoritative for one discovery interval even if a
/// newer transport/decode attempt failed. Future-dated, stale, malformed, missing, or internally
/// inconsistent evidence is `UNKNOWN`; it is never compressed into `UNPUBLISHED`.
pub fn product_term_publication(
    published: &PublishedDiscoverySnapshot,
    term: &TermId,
    now: OffsetDateTime,
) -> ServiceTermPublicationV2 {
    let Some(completed_at) = published
        .state
        .last_success
        .as_ref()
        .and_then(|success| success.completed_at.as_deref())
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
    else {
        return ServiceTermPublicationV2::Unknown;
    };
    let age = now - completed_at;
    let Ok(maximum_age) = time::Duration::try_from(DISCOVERY_REFRESH_INTERVAL) else {
        return ServiceTermPublicationV2::Unknown;
    };
    if age.is_negative() || age > maximum_age {
        return ServiceTermPublicationV2::Unknown;
    }

    let mut matching_terms = published
        .snapshot
        .terms
        .iter()
        .filter(|candidate| candidate.term_id == *term);
    let term_evidence = matching_terms.next();
    if matching_terms.next().is_some() {
        return ServiceTermPublicationV2::Unknown;
    }

    let matching_targets = published
        .snapshot
        .campuses
        .iter()
        .filter(|candidate| {
            candidate.target.term() == term && is_product_campus(candidate.target.campus().as_str())
        })
        .collect::<Vec<_>>();
    let unique_target_count = matching_targets
        .iter()
        .map(|candidate| candidate.target.campus().as_str().to_ascii_uppercase())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    if unique_target_count != matching_targets.len() {
        return ServiceTermPublicationV2::Unknown;
    }

    let term_published = match term_evidence {
        Some(evidence) => {
            let value =
                consistent_stored_bool(&evidence.canonical_facts, "published", evidence.published);
            if value == BoolEvidence::Indeterminate {
                return ServiceTermPublicationV2::Unknown;
            }
            Some(value)
        }
        None => None,
    };

    for target in &matching_targets {
        let campus_enabled = stored_bool_fact(&target.canonical_facts, "campusEnabled");
        let target_enabled =
            consistent_stored_bool(&target.canonical_facts, "targetEnabled", target.enabled);
        if campus_enabled == BoolEvidence::Indeterminate
            || target_enabled == BoolEvidence::Indeterminate
        {
            return ServiceTermPublicationV2::Unknown;
        }
    }

    if matching_targets.is_empty() {
        // A fresh, successfully validated full discovery document explicitly omits all three
        // product targets. This is the sole path to `UNPUBLISHED`.
        return ServiceTermPublicationV2::Unpublished;
    }

    match term_published {
        Some(BoolEvidence::True) => ServiceTermPublicationV2::Published,
        // A product target with an absent term or an explicitly unpublished term is internally
        // contradictory. Do not turn that conflict into an affirmative or negative claim.
        None | Some(BoolEvidence::False) | Some(BoolEvidence::Indeterminate) => {
            ServiceTermPublicationV2::Unknown
        }
    }
}

pub(crate) fn stored_presence_is_true(facts: &serde_json::Value, field: &str) -> bool {
    stored_bool_fact(facts, field) == BoolEvidence::True
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoolEvidence {
    True,
    False,
    Indeterminate,
}

fn stored_bool_fact(facts: &serde_json::Value, field: &str) -> BoolEvidence {
    let Some(presence) = facts.get(field) else {
        return BoolEvidence::Indeterminate;
    };
    if presence.get("state").and_then(serde_json::Value::as_str) != Some("VALUE") {
        return BoolEvidence::Indeterminate;
    }
    match presence.get("value").and_then(serde_json::Value::as_bool) {
        Some(true) => BoolEvidence::True,
        Some(false) => BoolEvidence::False,
        None => BoolEvidence::Indeterminate,
    }
}

fn consistent_stored_bool(
    facts: &serde_json::Value,
    field: &str,
    projected: Option<bool>,
) -> BoolEvidence {
    let stored = stored_bool_fact(facts, field);
    match (stored, projected) {
        (BoolEvidence::True, Some(true)) => BoolEvidence::True,
        (BoolEvidence::False, Some(false)) => BoolEvidence::False,
        _ => BoolEvidence::Indeterminate,
    }
}

#[cfg(test)]
mod tests {
    use bcsp_contracts::TraceId;
    use bcsp_operational_storage::{
        DiscoveredCampus, DiscoveredTerm, DiscoveryRefreshCommand, DiscoverySnapshot,
        DiscoverySourceKind, DiscoverySourceVersion, OperationalStorage,
    };

    use super::*;

    fn publication(completed_at: &str) -> PublishedDiscoverySnapshot {
        let term = TermId::try_from("72026").expect("term");
        let source_version_id =
            "selector:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let mut storage = OperationalStorage::open_in_memory().expect("storage");
        storage
            .apply_discovery_refresh(DiscoveryRefreshCommand {
                observation_id: "00000000-0000-4000-8000-000000000001"
                    .parse::<TraceId>()
                    .expect("trace"),
                started_at: "2026-07-17T05:59:58Z".to_owned(),
                completed_at: completed_at.to_owned(),
                snapshot: DiscoverySnapshot {
                    sources: vec![DiscoverySourceVersion {
                        source_version_id: source_version_id.to_owned(),
                        source_kind: DiscoverySourceKind::Selector,
                        source_identity: "RUTGERS_SELECTOR".to_owned(),
                        content_sha256: "a".repeat(64),
                        canonical_facts: serde_json::json!({"test": true}),
                        observed_at: "2026-07-17T05:59:58Z".to_owned(),
                    }],
                    terms: vec![DiscoveredTerm {
                        term_id: term.clone(),
                        year: Some(2026),
                        term_code: Some("7".to_owned()),
                        display_name: Some("Summer 2026".to_owned()),
                        published: Some(true),
                        canonical_facts: serde_json::json!({
                            "published": {"state": "VALUE", "value": true}
                        }),
                        source_version_id: source_version_id.to_owned(),
                    }],
                    campuses: vec![DiscoveredCampus {
                        target: TermCampusKey::new(
                            term,
                            CampusCode::try_from("NB").expect("campus"),
                        ),
                        display_name: Some("New Brunswick".to_owned()),
                        category: None,
                        enabled: Some(true),
                        canonical_facts: serde_json::json!({
                            "campusEnabled": {"state": "VALUE", "value": true},
                            "targetEnabled": {"state": "VALUE", "value": true}
                        }),
                        source_version_id: source_version_id.to_owned(),
                    }],
                    subjects: Vec::new(),
                },
            })
            .expect("publication");
        storage
            .published_discovery_snapshot()
            .expect("published snapshot")
    }

    #[test]
    fn product_scope_is_exact_and_case_insensitive_at_the_boundary() {
        assert_eq!(PRODUCT_CAMPUS_CODES, ["NB", "NK", "CM"]);
        assert!(is_product_campus("nb"));
        assert!(is_product_campus("NK"));
        assert!(is_product_campus("CM"));
        assert!(!is_product_campus("NWK"));
        assert!(!is_product_campus("CAM"));
        assert!(!is_product_campus("ONLINE_NB"));
    }

    #[test]
    fn one_term_expands_to_the_three_product_targets_in_stable_order() {
        let term = TermId::try_from("72026").expect("term");
        let targets = product_target_keys(&term);
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].campus().as_str(), "NB");
        assert_eq!(targets[1].campus().as_str(), "NK");
        assert_eq!(targets[2].campus().as_str(), "CM");
    }

    #[test]
    fn fresh_product_publication_distinguishes_present_and_explicit_all_absent() {
        let now = OffsetDateTime::parse("2026-07-17T12:00:00Z", &Rfc3339).expect("now");
        let term = TermId::try_from("72026").expect("term");
        assert_eq!(
            product_term_publication(&publication("2026-07-17T06:00:00Z"), &term, now,),
            ServiceTermPublicationV2::Published,
        );
        let mut explicit_all_absent = publication("2026-07-17T06:00:00Z");
        explicit_all_absent.snapshot.campuses.clear();
        assert_eq!(
            product_term_publication(&explicit_all_absent, &term, now),
            ServiceTermPublicationV2::Unpublished,
        );
        assert_eq!(
            product_term_publication(
                &publication("2026-07-17T06:00:00Z"),
                &TermId::try_from("02027").expect("later term"),
                now,
            ),
            ServiceTermPublicationV2::Unpublished,
        );
    }

    #[test]
    fn target_presence_is_publication_even_when_enabled_false() {
        let now = OffsetDateTime::parse("2026-07-17T12:00:00Z", &Rfc3339).expect("now");
        let term = TermId::try_from("72026").expect("term");
        let mut evidence = publication("2026-07-17T06:00:00Z");
        evidence.snapshot.campuses[0].enabled = Some(false);
        evidence.snapshot.campuses[0].canonical_facts["campusEnabled"] =
            serde_json::json!({"state": "VALUE", "value": false});
        evidence.snapshot.campuses[0].canonical_facts["targetEnabled"] =
            serde_json::json!({"state": "VALUE", "value": false});
        assert_eq!(
            product_term_publication(&evidence, &term, now),
            ServiceTermPublicationV2::Published,
        );
    }

    #[test]
    fn false_term_with_present_target_and_duplicate_target_are_unknown_conflicts() {
        let now = OffsetDateTime::parse("2026-07-17T12:00:00Z", &Rfc3339).expect("now");
        let term = TermId::try_from("72026").expect("term");
        let mut false_term = publication("2026-07-17T06:00:00Z");
        false_term.snapshot.terms[0].published = Some(false);
        false_term.snapshot.terms[0].canonical_facts["published"] =
            serde_json::json!({"state": "VALUE", "value": false});
        assert_eq!(
            product_term_publication(&false_term, &term, now),
            ServiceTermPublicationV2::Unknown,
        );

        let mut duplicate = publication("2026-07-17T06:00:00Z");
        duplicate
            .snapshot
            .campuses
            .push(duplicate.snapshot.campuses[0].clone());
        assert_eq!(
            product_term_publication(&duplicate, &term, now),
            ServiceTermPublicationV2::Unknown,
        );
    }

    #[test]
    fn stale_future_and_missing_success_evidence_are_unknown() {
        let now = OffsetDateTime::parse("2026-07-17T12:00:00Z", &Rfc3339).expect("now");
        let term = TermId::try_from("72026").expect("term");
        assert_eq!(
            product_term_publication(&publication("2026-07-17T05:59:59Z"), &term, now,),
            ServiceTermPublicationV2::Unknown,
        );
        assert_eq!(
            product_term_publication(&publication("2026-07-17T12:00:01Z"), &term, now,),
            ServiceTermPublicationV2::Unknown,
        );
        let mut no_success = publication("2026-07-17T06:00:00Z");
        no_success.state.last_success = None;
        no_success.state.last_success_observation_id = None;
        assert_eq!(
            product_term_publication(&no_success, &term, now),
            ServiceTermPublicationV2::Unknown,
        );
    }

    #[test]
    fn missing_null_malformed_and_conflicting_fields_are_unknown() {
        let now = OffsetDateTime::parse("2026-07-17T12:00:00Z", &Rfc3339).expect("now");
        let term = TermId::try_from("72026").expect("term");
        for state in ["MISSING", "EXPLICIT_NULL", "MALFORMED"] {
            let mut evidence = publication("2026-07-17T06:00:00Z");
            evidence.snapshot.campuses[0].canonical_facts["campusEnabled"] =
                serde_json::json!({"state": state});
            assert_eq!(
                product_term_publication(&evidence, &term, now),
                ServiceTermPublicationV2::Unknown,
                "{state}",
            );
        }

        let mut conflicting = publication("2026-07-17T06:00:00Z");
        conflicting.snapshot.campuses[0].canonical_facts["targetEnabled"] =
            serde_json::json!({"state": "VALUE", "value": false});
        assert_eq!(
            product_term_publication(&conflicting, &term, now),
            ServiceTermPublicationV2::Unknown,
        );

        let mut missing_term_fact = publication("2026-07-17T06:00:00Z");
        missing_term_fact.snapshot.terms[0].published = None;
        missing_term_fact.snapshot.terms[0].canonical_facts["published"] =
            serde_json::json!({"state": "MISSING"});
        assert_eq!(
            product_term_publication(&missing_term_fact, &term, now),
            ServiceTermPublicationV2::Unknown,
        );
    }
}
