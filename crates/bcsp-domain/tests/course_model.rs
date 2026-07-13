use bcsp_contracts::{CourseGroupKey, CourseVariantKey, SectionKey};
use bcsp_domain::{CourseGroup, CourseVariant, DomainModelError, DomainReasonCode};

fn group_key(campus: &str) -> CourseGroupKey {
    CourseGroupKey::try_new("T2026F", campus, "00:000:001").unwrap()
}

fn variant_key(group: CourseGroupKey, digest: char) -> CourseVariantKey {
    CourseVariantKey::try_new(group, &format!("v1:{}", digest.to_string().repeat(64))).unwrap()
}

fn section(campus: &str, index: &str) -> SectionKey {
    SectionKey::try_new("T2026F", campus, index).unwrap()
}

#[test]
fn valid_group_keeps_sections_attached_to_exactly_one_variant() {
    let group = group_key("CAMPUS_A");
    let first = CourseVariant::try_new(
        variant_key(group.clone(), 'a'),
        vec![section("CAMPUS_A", "00002"), section("CAMPUS_A", "00001")],
    )
    .unwrap();
    let second = CourseVariant::try_new(
        variant_key(group.clone(), 'b'),
        vec![section("CAMPUS_A", "00003")],
    )
    .unwrap();
    let model = CourseGroup::try_new(group, vec![second, first]).unwrap();

    assert_eq!(model.variants().len(), 2);
    assert_eq!(
        model.variants()[0].section_keys()[0].index().as_str(),
        "00001"
    );
}

#[test]
fn model_invariants_fail_closed_with_stable_reason_codes() {
    let group = group_key("CAMPUS_A");
    let cross_scope = CourseVariant::try_new(
        variant_key(group.clone(), 'a'),
        vec![section("CAMPUS_B", "00001")],
    )
    .unwrap_err();
    assert_eq!(cross_scope, DomainModelError::SectionScopeMismatch);
    assert_eq!(
        cross_scope.reason_code(),
        DomainReasonCode::SectionScopeMismatch
    );

    let duplicate_section = section("CAMPUS_A", "00001");
    assert_eq!(
        CourseVariant::try_new(
            variant_key(group.clone(), 'a'),
            vec![duplicate_section.clone(), duplicate_section]
        )
        .unwrap_err(),
        DomainModelError::DuplicateSection
    );

    assert_eq!(
        CourseGroup::try_new(group.clone(), Vec::new()).unwrap_err(),
        DomainModelError::EmptyCourseGroup
    );

    let mismatched = CourseVariant::try_new(
        variant_key(group_key("CAMPUS_B"), 'a'),
        vec![section("CAMPUS_B", "00001")],
    )
    .unwrap();
    assert_eq!(
        CourseGroup::try_new(group, vec![mismatched]).unwrap_err(),
        DomainModelError::VariantGroupMismatch
    );
}

#[test]
fn section_overlap_between_variants_is_rejected() {
    let group = group_key("CAMPUS_A");
    let shared = section("CAMPUS_A", "00001");
    let first =
        CourseVariant::try_new(variant_key(group.clone(), 'a'), vec![shared.clone()]).unwrap();
    let second = CourseVariant::try_new(variant_key(group.clone(), 'b'), vec![shared]).unwrap();
    assert_eq!(
        CourseGroup::try_new(group, vec![first, second]).unwrap_err(),
        DomainModelError::SectionAcrossVariants
    );
}

#[test]
fn duplicate_variant_fingerprint_is_rejected() {
    let group = group_key("CAMPUS_A");
    let key = variant_key(group.clone(), 'a');
    let first = CourseVariant::try_new(key.clone(), vec![section("CAMPUS_A", "00001")]).unwrap();
    let second = CourseVariant::try_new(key, vec![section("CAMPUS_A", "00002")]).unwrap();
    assert_eq!(
        CourseGroup::try_new(group, vec![first, second]).unwrap_err(),
        DomainModelError::DuplicateVariantFingerprint
    );
}
