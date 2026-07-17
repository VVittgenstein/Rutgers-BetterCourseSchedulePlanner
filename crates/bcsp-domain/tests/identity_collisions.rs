use std::collections::BTreeSet;

use bcsp_contracts::SectionKey;
use bcsp_domain::{
    DomainModelError, DomainReasonCode, DuplicateDisposition, SectionDuplicate,
    classify_section_duplicates,
};

struct IdentityFixture {
    data_classification: String,
    section_keys: Vec<SectionKey>,
    equivalent_duplicate_pairs: Vec<Vec<SectionDuplicate<String>>>,
    conflicting_duplicate: Vec<SectionDuplicate<String>>,
    mixed_key_duplicate: Vec<SectionDuplicate<String>>,
}

fn fixture() -> IdentityFixture {
    let mut data_classification = None;
    let mut section_keys = Vec::new();
    let mut equivalent_duplicate_pairs = Vec::new();
    let mut conflicting_duplicate = Vec::new();
    let mut mixed_key_duplicate = Vec::new();
    for line in include_str!("fixtures/synthetic-identity-collisions-v1.tsv").lines() {
        let columns = line.split('\t').collect::<Vec<_>>();
        match columns.as_slice() {
            ["classification", value] => data_classification = Some((*value).to_owned()),
            ["section", term, campus, index] => section_keys.push(
                SectionKey::try_new(term, campus, index).expect("valid synthetic section fixture"),
            ),
            ["equivalent", term, campus, index, first, second] => {
                let key = SectionKey::try_new(term, campus, index).unwrap();
                equivalent_duplicate_pairs.push(vec![
                    SectionDuplicate::new(key.clone(), (*first).to_owned()),
                    SectionDuplicate::new(key, (*second).to_owned()),
                ]);
            }
            ["conflicting", term, campus, index, first, second] => {
                let key = SectionKey::try_new(term, campus, index).unwrap();
                conflicting_duplicate = vec![
                    SectionDuplicate::new(key.clone(), (*first).to_owned()),
                    SectionDuplicate::new(key, (*second).to_owned()),
                ];
            }
            [
                "mixed",
                term,
                first_campus,
                second_campus,
                index,
                fingerprint,
            ] => {
                mixed_key_duplicate = vec![
                    SectionDuplicate::new(
                        SectionKey::try_new(term, first_campus, index).unwrap(),
                        (*fingerprint).to_owned(),
                    ),
                    SectionDuplicate::new(
                        SectionKey::try_new(term, second_campus, index).unwrap(),
                        (*fingerprint).to_owned(),
                    ),
                ];
            }
            _ => panic!("invalid synthetic fixture row"),
        }
    }
    IdentityFixture {
        data_classification: data_classification.expect("fixture classification"),
        section_keys,
        equivalent_duplicate_pairs,
        conflicting_duplicate,
        mixed_key_duplicate,
    }
}

#[test]
fn complete_section_key_prevents_cross_scope_collisions() {
    let fixture = fixture();
    assert_eq!(fixture.data_classification, "SYNTHETIC_NO_REAL_COURSE_DATA");
    assert_eq!(fixture.section_keys.len(), 3);
    assert_eq!(
        fixture
            .section_keys
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(
        fixture.section_keys[0].index(),
        fixture.section_keys[1].index()
    );
    assert_ne!(fixture.section_keys[0], fixture.section_keys[1]);
    assert_ne!(fixture.section_keys[0], fixture.section_keys[2]);
    assert_eq!(fixture.section_keys[0].to_string(), "T2026F/CAMPUS_A/00001");
    assert_eq!(
        "T2026F/CAMPUS_A/00001".parse::<SectionKey>().unwrap(),
        fixture.section_keys[0]
    );
}

#[test]
fn duplicate_classification_binds_complete_identity_and_fails_closed() {
    let fixture = fixture();
    assert_eq!(fixture.equivalent_duplicate_pairs.len(), 18);
    for pair in fixture.equivalent_duplicate_pairs {
        assert_eq!(
            classify_section_duplicates(pair).unwrap(),
            DuplicateDisposition::AuditedEquivalentCollapse
        );
    }

    let conflict = classify_section_duplicates(fixture.conflicting_duplicate).unwrap_err();
    assert_eq!(conflict, DomainModelError::ConflictingDuplicate);
    assert_eq!(
        conflict.reason_code(),
        DomainReasonCode::ConflictingDuplicate
    );

    let mixed = classify_section_duplicates(fixture.mixed_key_duplicate).unwrap_err();
    assert_eq!(mixed, DomainModelError::DuplicateKeyMismatch);
    assert_eq!(mixed.reason_code(), DomainReasonCode::DuplicateKeyMismatch);

    let unique = SectionDuplicate::new(
        SectionKey::try_new("T2026F", "CAMPUS_A", "00020").unwrap(),
        "only-one".to_owned(),
    );
    assert_eq!(
        classify_section_duplicates([unique]).unwrap(),
        DuplicateDisposition::Unique
    );
    assert!(
        classify_section_duplicates::<String, _>(Vec::<SectionDuplicate<String>>::new()).is_err()
    );
}
