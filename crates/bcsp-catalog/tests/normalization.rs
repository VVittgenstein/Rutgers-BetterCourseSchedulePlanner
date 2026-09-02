use bcsp_catalog::{
    CollectionPresence, DayKnowledge, DeliveryModality, NormalizationError, OccurrenceKind,
    OccurrenceModality, Synchronicity, TimeKnowledge, Weekday, normalize_target,
    variant_fingerprint_v1,
};
use bcsp_contracts::TermCampusKey;
use bcsp_rutgers_client::{Presence, RawCatalogCourse, SourceProvenance, decode_catalog_payload};
use serde_json::{Value, json};

const SYNTHETIC_FIXTURE_ONLY: &str = "SYNTHETIC_NO_REAL_COURSE_DATA";

fn normalize(
    body: &[u8],
    term: &str,
    campus: &str,
    observed_at: &str,
) -> Result<bcsp_catalog::NormalizedCatalogTarget, NormalizationError> {
    assert_eq!(SYNTHETIC_FIXTURE_ONLY, "SYNTHETIC_NO_REAL_COURSE_DATA");
    let target = TermCampusKey::try_new(term, campus).expect("synthetic target must be valid");
    let raw = decode_catalog_payload(body).expect("synthetic fixture must decode");
    normalize_target(
        target,
        raw,
        SourceProvenance::from_body("SYNTHETIC", observed_at, body),
    )
}

#[test]
fn groups_multiple_offering_variants_without_last_write_wins() {
    let body = br#"[
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","subject":"SYN","courseNumber":"001",
        "supplementCode":"LB","creditsObject":{"code":"0"},"title":"SYNTHETIC A",
        "expandedTitle":"SYNTHETIC A","level":"U","offeringUnitCode":"01",
        "sections":[{"campusCode":"SYN","index":"10001","number":"01","sectionCourseType":"T","meetingTimes":[],"instructors":[]}]
      },
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","subject":"SYN","courseNumber":"001",
        "supplementCode":"","creditsObject":{"code":"4"},"title":"SYNTHETIC A",
        "expandedTitle":"SYNTHETIC A","level":"U","offeringUnitCode":"01",
        "sections":[{"campusCode":"SYN","index":"10002","number":"02","sectionCourseType":"O","meetingTimes":[],"instructors":[]}]
      }
    ]"#;

    let target = normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("distinct variants should normalize");
    assert_eq!(target.groups.len(), 1);
    assert_eq!(target.groups[0].raw_course_count, 2);
    assert_eq!(target.groups[0].variants.len(), 2);
    assert_eq!(target.metrics.raw_sections, 2);
    assert_eq!(target.metrics.normalized_sections, 2);
}

#[test]
fn equivalent_order_only_section_duplicates_collapse_with_audit() {
    let body = br#"[
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","supplementCode":"","creditsObject":{"code":"3"},
        "title":"SYNTHETIC","expandedTitle":"SYNTHETIC","level":"U","offeringUnitCode":"01",
        "sections":[{"campusCode":"SYN","index":"00001","comments":["SYN_A","SYN_B"],"commentsText":"SYN_A SYN_B","instructors":[{"name":"SYNTHETIC_INSTRUCTOR_A"},{"name":"SYNTHETIC_INSTRUCTOR_B"}],"instructorsText":"SYNTHETIC_INSTRUCTOR_A, SYNTHETIC_INSTRUCTOR_B","meetingTimes":[]}]
      },
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","supplementCode":"","creditsObject":{"code":"3"},
        "title":"SYNTHETIC","expandedTitle":"SYNTHETIC","level":"U","offeringUnitCode":"01",
        "sections":[{"campusCode":"SYN","index":"00001","comments":["SYN_B","SYN_A"],"commentsText":"SYN_B SYN_A","instructors":[{"name":"SYNTHETIC_INSTRUCTOR_B"},{"name":"SYNTHETIC_INSTRUCTOR_A"}],"instructorsText":"SYNTHETIC_INSTRUCTOR_B, SYNTHETIC_INSTRUCTOR_A","meetingTimes":[]}]
      }
    ]"#;

    let target = normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("equivalent duplicate should collapse");
    assert_eq!(target.metrics.raw_sections, 2);
    assert_eq!(target.metrics.normalized_sections, 1);
    assert_eq!(target.metrics.equivalent_duplicate_keys, 1);
    assert_eq!(target.duplicate_audits[0].raw_record_count, 2);
    assert_eq!(target.sections().count(), 1);
}

#[test]
fn equivalent_real_shape_comment_object_duplicates_collapse_with_audit() {
    let body = br#"[
          {
            "campusCode":"SYN",
            "courseString":"SYN:CAT:COMMENT_OBJECT",
            "sections":[{
              "campusCode":"SYN",
              "index":"00002",
              "comments":[
                {"code":" A ","description":" First "},
                {"code":"B","description":"Second"}
              ],
              "commentsText":"First Second",
              "meetingTimes":[]
            },{
              "campusCode":"SYN",
              "index":"00002",
              "comments":[
                {"code":"B","description":"Second"},
                {"code":"A","description":"First"}
              ],
              "commentsText":"Second First",
              "meetingTimes":[]
            }]
          }
        ]"#;
    let target = normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("equivalent comment-object duplicate should collapse");

    assert_eq!(target.metrics.raw_sections, 2);
    assert_eq!(target.metrics.normalized_sections, 1);
    assert_eq!(target.metrics.equivalent_duplicate_keys, 1);
}

#[test]
fn equivalent_course_objects_are_input_order_independent_for_serving_fields() {
    let forward = br#"[
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","title":" SYNTHETIC ",
        "coreCodes":["SYN_B","SYN_A"],"campusLocations":[{"code":"SYN_B"},{"code":"SYN_A"}],
        "openSections":1,"sections":[{"campusCode":"SYN","index":"50001"}]
      },
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","title":"SYNTHETIC",
        "coreCodes":["SYN_A","SYN_B"],"campusLocations":[{"code":"SYN_A"},{"code":"SYN_B"}],
        "openSections":99,"sections":[{"campusCode":"SYN","index":"50002"}]
      }
    ]"#;
    let reverse = br#"[
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","title":"SYNTHETIC",
        "coreCodes":["SYN_A","SYN_B"],"campusLocations":[{"code":"SYN_A"},{"code":"SYN_B"}],
        "openSections":99,"sections":[{"campusCode":"SYN","index":"50002"}]
      },
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","title":" SYNTHETIC ",
        "coreCodes":["SYN_B","SYN_A"],"campusLocations":[{"code":"SYN_B"},{"code":"SYN_A"}],
        "openSections":1,"sections":[{"campusCode":"SYN","index":"50001"}]
      }
    ]"#;

    let forward = normalize(forward, "T2030", "SYN", "2030-01-01T00:00:00Z").unwrap();
    let reverse = normalize(reverse, "T2030", "SYN", "2031-01-01T00:00:00Z").unwrap();
    let forward_variant = &forward.groups[0].variants[0];
    let reverse_variant = &reverse.groups[0].variants[0];
    assert_eq!(forward_variant.fields, reverse_variant.fields);
    assert_eq!(
        forward_variant.fields.title,
        Presence::Value("SYNTHETIC".to_owned())
    );
    assert_eq!(
        forward_variant.fields.core_codes,
        Presence::Value(vec![json!("SYN_A"), json!("SYN_B")])
    );
    assert_eq!(
        forward_variant.canonical_course_facts,
        reverse_variant.canonical_course_facts
    );
    assert_eq!(forward_variant.record_hashes, reverse_variant.record_hashes);
    assert_eq!(forward.content_hash, reverse.content_hash);
    assert_eq!(
        forward
            .sections()
            .map(|section| section.key.clone())
            .collect::<Vec<_>>(),
        reverse
            .sections()
            .map(|section| section.key.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn equivalent_section_representative_and_normalized_sets_are_input_order_independent() {
    let forward = br#"[{
      "campusCode":"SYN","courseString":"SYN:CAT:001","sections":[
        {"campusCode":"SYN","index":"50003","comments":[" B ","A"],"commentsText":"B A","instructors":[{"name":" B "},{"name":"A"}],"instructorsText":"B, A"},
        {"campusCode":"SYN","index":"50003","comments":["A","B"],"commentsText":"A B","instructors":[{"name":"A"},{"name":"B"}],"instructorsText":"A, B"}
      ]
    }]"#;
    let reverse = br#"[{
      "campusCode":"SYN","courseString":"SYN:CAT:001","sections":[
        {"campusCode":"SYN","index":"50003","comments":["A","B"],"commentsText":"A B","instructors":[{"name":"A"},{"name":"B"}],"instructorsText":"A, B"},
        {"campusCode":"SYN","index":"50003","comments":[" B ","A"],"commentsText":"B A","instructors":[{"name":" B "},{"name":"A"}],"instructorsText":"B, A"}
      ]
    }]"#;

    let forward = normalize(forward, "T2030", "SYN", "2030-01-01T00:00:00Z").unwrap();
    let reverse = normalize(reverse, "T2030", "SYN", "2031-01-01T00:00:00Z").unwrap();
    let forward_section = forward.sections().next().unwrap();
    let reverse_section = reverse.sections().next().unwrap();
    assert_eq!(
        forward_section.first_record_hash,
        reverse_section.first_record_hash
    );
    assert_eq!(
        forward_section.canonical_facts,
        reverse_section.canonical_facts
    );
    assert_eq!(forward_section.instructors, reverse_section.instructors);
    assert_eq!(forward_section.occurrences, reverse_section.occurrences);
    assert_eq!(forward.content_hash, reverse.content_hash);
}

#[test]
fn derived_text_differences_conflict_without_a_trustworthy_structured_array() {
    for structured in [None, Some(Value::Null), Some(json!("MALFORMED_SYNTHETIC"))] {
        let mut left = json!({
            "campusCode":"SYN","index":"50004","commentsText":"SYNTHETIC_A"
        });
        let mut right = json!({
            "campusCode":"SYN","index":"50004","commentsText":"SYNTHETIC_B"
        });
        if let Some(value) = structured {
            left.as_object_mut()
                .unwrap()
                .insert("comments".into(), value.clone());
            right
                .as_object_mut()
                .unwrap()
                .insert("comments".into(), value);
        }
        let body = serde_json::to_vec(&json!([{
            "campusCode":"SYN","courseString":"SYN:CAT:001","sections":[left,right]
        }]))
        .unwrap();
        assert!(matches!(
            normalize(&body, "T2030", "SYN", "2030-01-01T00:00:00Z"),
            Err(NormalizationError::ConflictingDuplicate { .. })
        ));
    }

    for (structured_field, structured_value, derived_field) in [
        ("instructors", json!([{"name":7}]), "instructorsText"),
        ("instructors", json!([{}]), "instructorsText"),
        (
            "crossListedSections",
            json!([{}]),
            "crossListedSectionsText",
        ),
    ] {
        let mut left = json!({"campusCode":"SYN","index":"50004"});
        let mut right = json!({"campusCode":"SYN","index":"50004"});
        for (section, text) in [(&mut left, "SYNTHETIC_A"), (&mut right, "SYNTHETIC_B")] {
            let object = section.as_object_mut().unwrap();
            object.insert(structured_field.to_owned(), structured_value.clone());
            object.insert(derived_field.to_owned(), json!(text));
        }
        let body = serde_json::to_vec(&json!([{
            "campusCode":"SYN","courseString":"SYN:CAT:001","sections":[left,right]
        }]))
        .unwrap();
        assert!(matches!(
            normalize(&body, "T2030", "SYN", "2030-01-01T00:00:00Z"),
            Err(NormalizationError::ConflictingDuplicate { .. })
        ));
    }
}

#[test]
fn unknown_ordered_arrays_and_meeting_ordinals_fail_closed_as_conflicts() {
    for (left, right) in [
        (
            json!({"future":{"comments":["B","A"],"title":" SYNTHETIC "}}),
            json!({"future":{"comments":["A","B"],"title":"SYNTHETIC"}}),
        ),
        (
            json!({"meetingTimes":[{"meetingModeCode":"92"},{"meetingModeCode":"93"}]}),
            json!({"meetingTimes":[{"meetingModeCode":"93"},{"meetingModeCode":"92"}]}),
        ),
    ] {
        let mut left_section = json!({"campusCode":"SYN","index":"50005"});
        let mut right_section = json!({"campusCode":"SYN","index":"50005"});
        left_section
            .as_object_mut()
            .unwrap()
            .extend(left.as_object().unwrap().clone());
        right_section
            .as_object_mut()
            .unwrap()
            .extend(right.as_object().unwrap().clone());
        let body = serde_json::to_vec(&json!([{
            "campusCode":"SYN","courseString":"SYN:CAT:001",
            "sections":[left_section,right_section]
        }]))
        .unwrap();
        assert!(matches!(
            normalize(&body, "T2030", "SYN", "2030-01-01T00:00:00Z"),
            Err(NormalizationError::ConflictingDuplicate { .. })
        ));
    }
}

#[test]
fn contradictory_duplicate_fails_the_entire_target() {
    let body = br#"[
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","supplementCode":"","creditsObject":{"code":"3"},
        "title":"SYNTHETIC","expandedTitle":"SYNTHETIC","level":"U","offeringUnitCode":"01",
        "sections":[
          {"campusCode":"SYN","index":"00002","number":"01","sectionCourseType":"T"},
          {"campusCode":"SYN","index":"00002","number":"02","sectionCourseType":"O"}
        ]
      }
    ]"#;

    assert!(matches!(
        normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z"),
        Err(NormalizationError::ConflictingDuplicate { .. })
    ));
}

#[test]
fn section_reuse_across_variants_fails_closed() {
    let body = br#"[
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","supplementCode":"A","creditsObject":{"code":"3"},
        "title":"SYNTHETIC","expandedTitle":"SYNTHETIC","level":"U","offeringUnitCode":"01",
        "sections":[{"campusCode":"SYN","index":"00003","number":"01"}]
      },
      {
        "campusCode":"SYN","courseString":"SYN:CAT:001","supplementCode":"B","creditsObject":{"code":"4"},
        "title":"SYNTHETIC","expandedTitle":"SYNTHETIC","level":"U","offeringUnitCode":"01",
        "sections":[{"campusCode":"SYN","index":"00003","number":"01"}]
      }
    ]"#;

    assert!(matches!(
        normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z"),
        Err(NormalizationError::SectionAcrossVariants { .. })
    ));
}

#[test]
fn section_identity_always_includes_term_and_campus() {
    let template = |campus: &str| {
        format!(
            r#"[{{"campusCode":"{campus}","courseString":"SYN:CAT:001","sections":[{{"campusCode":"{campus}","index":"12345"}}]}}]"#
        )
    };
    let primary_body = template("SYN");
    let alternate_body = template("ALT");
    let primary = normalize(
        primary_body.as_bytes(),
        "T2030",
        "SYN",
        "2030-01-01T00:00:00Z",
    )
    .expect("synthetic primary target should normalize");
    let alternate = normalize(
        alternate_body.as_bytes(),
        "T2030",
        "ALT",
        "2030-01-01T00:00:00Z",
    )
    .expect("synthetic alternate target should normalize");
    assert_ne!(
        primary.sections().next().unwrap().key,
        alternate.sections().next().unwrap().key
    );

    let selector_scoped = normalize(
        template("ALT").as_bytes(),
        "T2030",
        "SYN",
        "2030-01-01T00:00:00Z",
    )
    .expect("selector scope may contain a different upstream campus code");
    let variant = &selector_scoped.groups[0].variants[0];
    let section = &variant.sections[0];
    assert_eq!(selector_scoped.target.campus().as_str(), "SYN");
    assert_eq!(variant.key.group().campus().as_str(), "SYN");
    assert_eq!(section.key.campus().as_str(), "SYN");
    assert!(variant.canonical_course_facts.get("campusCode").is_none());
    assert_eq!(section.canonical_facts["campusCode"], "ALT");
}

#[test]
fn selector_scoped_course_objects_merge_location_sets_without_false_variants() {
    let body = br#"[
      {
        "campusCode":"SYN_A","campusLocations":[{"code":"SYN_A"}],
        "courseString":"SYN:CAT:001","title":"SYNTHETIC",
        "sections":[{"campusCode":"SYN_A","index":"12345"}]
      },
      {
        "campusCode":"SYN_B","campusLocations":[{"code":"SYN_B"}],
        "courseString":"SYN:CAT:001","title":"SYNTHETIC",
        "sections":[{"campusCode":"SYN_B","index":"12346"}]
      }
    ]"#;
    let normalized = normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("scope-only course splits should merge deterministically");
    let group = &normalized.groups[0];
    assert_eq!(group.raw_course_count, 2);
    assert_eq!(group.variants.len(), 1);
    let variant = &group.variants[0];
    assert_eq!(variant.raw_course_count, 2);
    assert_eq!(variant.sections.len(), 2);
    assert_eq!(
        variant.canonical_course_facts["campusLocations"],
        json!([{"code":"SYN_A"},{"code":"SYN_B"}])
    );
}

#[test]
fn malformed_course_or_section_campus_still_fails_shape_validation() {
    let malformed_course = br#"[{
      "campusCode":7,"courseString":"SYN:CAT:001","sections":[]
    }]"#;
    assert!(matches!(
        normalize(malformed_course, "T2030", "SYN", "2030-01-01T00:00:00Z"),
        Err(NormalizationError::MalformedRequiredField(
            "course.campusCode"
        ))
    ));

    let malformed_section = br#"[{
      "campusCode":"SYN","courseString":"SYN:CAT:001",
      "sections":[{"campusCode":7,"index":"12345"}]
    }]"#;
    assert!(matches!(
        normalize(malformed_section, "T2030", "SYN", "2030-01-01T00:00:00Z"),
        Err(NormalizationError::MalformedRequiredField(
            "section.campusCode"
        ))
    ));
}

#[test]
fn delivery_registry_preserves_t_h_o_unknown_and_conflicts() {
    let body = br#"[{
      "campusCode":"SYN","courseString":"SYN:CAT:001","sections":[
        {"campusCode":"SYN","index":"10001","sectionCourseType":"T","meetingTimes":[{"meetingModeCode":"02","meetingDay":"TH","startTimeMilitary":"0900","endTimeMilitary":"1020","campusLocation":"SYN"}]},
        {"campusCode":"SYN","index":"10002","sectionCourseType":"H","meetingTimes":[{"meetingModeCode":"91","meetingDay":"M","startTimeMilitary":"1000","endTimeMilitary":"1120"}]},
        {"campusCode":"SYN","index":"10003","sectionCourseType":"O","meetingTimes":[{"meetingModeCode":"90","meetingDay":"","startTimeMilitary":"","endTimeMilitary":"","baClassHours":"B","campusLocation":"O"}]},
        {"campusCode":"SYN","index":"10004","sectionCourseType":"O","meetingTimes":[{"meetingModeCode":"92","meetingDay":"W","startTimeMilitary":"1800","endTimeMilitary":"2120","campusLocation":"T"}]},
        {"campusCode":"SYN","index":"10005","sectionCourseType":"O","meetingTimes":[{"meetingModeCode":"93","meetingDay":null,"startTimeMilitary":null,"endTimeMilitary":null,"campusLocation":"O"}]},
        {"campusCode":"SYN","index":"10006","sectionCourseType":"X","meetingTimes":[{"meetingModeCode":"ZZ","meetingDay":"TTH","startTimeMilitary":"0900","endTimeMilitary":"1020"}]},
        {"campusCode":"SYN","index":"10007","sectionCourseType":"T","meetingTimes":[{"meetingModeCode":"90","campusLocation":"O"}]},
        {"campusCode":"SYN","index":"10008","sectionCourseType":"O","meetingTimes":[{"meetingModeCode":"02","campusLocation":"SYN"}]},
        {"campusCode":"SYN","index":"10009","sectionCourseType":"T","meetingTimes":[{"meetingModeCode":"ZZ","campusLocation":"SYN"}]},
        {"campusCode":"SYN","index":"10010","sectionCourseType":"O","meetingTimes":[{"meetingModeCode":"ZZ","campusLocation":"O"}]},
        {"campusCode":"SYN","index":"10011","sectionCourseType":"O","meetingTimes":[{"meetingModeCode":"90","meetingDay":"M","startTimeMilitary":"0900","endTimeMilitary":"1020","campusLocation":"O"}]}
      ]
    }]"#;
    let target = normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("delivery fixture should normalize");
    let sections = target.sections().collect::<Vec<_>>();

    assert_eq!(
        sections[0].delivery_modality,
        DeliveryModality::OnCampusOrInPerson
    );
    assert_eq!(sections[1].delivery_modality, DeliveryModality::Hybrid);
    assert_eq!(sections[2].delivery_modality, DeliveryModality::Online);
    assert_eq!(
        sections[2].occurrences[0].modality,
        OccurrenceModality::Online
    );
    // Mode 90 with "hours by arrangement" and no time is asynchronous.
    assert_eq!(sections[2].synchronicity, Synchronicity::Asynchronous);
    assert_eq!(
        sections[2].occurrences[0].normalization_reason,
        "ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS"
    );
    assert_eq!(sections[3].synchronicity, Synchronicity::Synchronous);
    assert_eq!(sections[4].synchronicity, Synchronicity::Asynchronous);
    // Mode 90 without a time and without "B" stays conservative.
    assert_eq!(sections[6].synchronicity, Synchronicity::Unspecified);
    assert_eq!(
        sections[6].occurrences[0].normalization_reason,
        "GENERIC_ONLINE_UNSPECIFIED"
    );
    // Mode 90 with a scheduled day/time is synchronous.
    assert_eq!(sections[10].delivery_modality, DeliveryModality::Online);
    assert_eq!(sections[10].synchronicity, Synchronicity::Synchronous);
    assert_eq!(
        sections[10].occurrences[0].normalization_reason,
        "ONLINE_SCHEDULED_SYNCHRONOUS"
    );
    assert_eq!(sections[5].delivery_modality, DeliveryModality::Unknown);
    assert_eq!(
        sections[5].occurrences[0].modality,
        OccurrenceModality::Unknown
    );
    assert_eq!(
        sections[6].delivery_modality,
        DeliveryModality::UnknownConflict
    );
    assert_eq!(
        sections[7].delivery_modality,
        DeliveryModality::UnknownConflict
    );
    assert_eq!(
        sections[8].occurrences[0].modality,
        OccurrenceModality::Unknown
    );
    assert_eq!(
        sections[9].occurrences[0].modality,
        OccurrenceModality::Unknown
    );
    assert_eq!(target.metrics.delivery_conflict_raw_sections, 2);

    let DayKnowledge::Known(days) = &sections[5].occurrences[0].days else {
        panic!("TTH should parse");
    };
    assert_eq!(days.as_slice(), &[Weekday::Tuesday, Weekday::Thursday]);
    assert_eq!(
        sections[0].occurrences[0].time,
        TimeKnowledge::Known {
            start_minute: 9 * 60,
            end_minute: 10 * 60 + 20,
        }
    );
}

#[test]
fn normalized_collections_keep_missing_null_and_empty_distinct() {
    let body = br#"[{
      "campusCode":"SYN","courseString":"SYN:CAT:001","sections":[
        {"campusCode":"SYN","index":"20001"},
        {"campusCode":"SYN","index":"20002","meetingTimes":null,"instructors":null},
        {"campusCode":"SYN","index":"20003","meetingTimes":[],"instructors":[]}
      ]
    }]"#;
    let target = normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("presence fixture should normalize");
    let sections = target.sections().collect::<Vec<_>>();
    assert_eq!(sections[0].occurrence_source, CollectionPresence::Missing);
    assert_eq!(sections[1].occurrence_source, CollectionPresence::Null);
    assert_eq!(
        sections[2].occurrence_source,
        CollectionPresence::PresentEmpty
    );
    assert_eq!(sections[0].instructor_source, CollectionPresence::Missing);
    assert_eq!(sections[1].instructor_source, CollectionPresence::Null);
    assert_eq!(
        sections[2].instructor_source,
        CollectionPresence::PresentEmpty
    );
}

#[test]
fn missing_null_or_malformed_core_sections_fail_target_but_empty_is_valid() {
    for body in [
        br#"[{"campusCode":"SYN","courseString":"SYN:CAT:001"}]"#.as_slice(),
        br#"[{"campusCode":"SYN","courseString":"SYN:CAT:001","sections":null}]"#.as_slice(),
        br#"[{"campusCode":"SYN","courseString":"SYN:CAT:001","sections":"MALFORMED_SYNTHETIC"}]"#
            .as_slice(),
    ] {
        assert!(matches!(
            normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z"),
            Err(NormalizationError::MissingRequiredField("course.sections"))
                | Err(NormalizationError::MalformedRequiredField(
                    "course.sections"
                ))
        ));
    }

    let valid_empty = br#"[{"campusCode":"SYN","courseString":"SYN:CAT:001","sections":[]}]"#;
    let normalized = normalize(valid_empty, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("present empty sections is a known empty collection");
    assert_eq!(normalized.metrics.raw_courses, 1);
    assert_eq!(normalized.metrics.raw_sections, 0);
}

#[test]
fn content_hash_excludes_observation_metadata_and_input_order() {
    let left = br#"[
      {"campusCode":"SYN","courseString":"SYN:CAT:002","sections":[{"campusCode":"SYN","index":"30002"}]},
      {"campusCode":"SYN","courseString":"SYN:CAT:001","sections":[{"campusCode":"SYN","index":"30001"}]}
    ]"#;
    let right = br#"[
      {"campusCode":"SYN","courseString":"SYN:CAT:001","sections":[{"campusCode":"SYN","index":"30001"}]},
      {"campusCode":"SYN","courseString":"SYN:CAT:002","sections":[{"campusCode":"SYN","index":"30002"}]}
    ]"#;
    let left = normalize(left, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("left fixture should normalize");
    let right = normalize(right, "T2030", "SYN", "2031-01-01T00:00:00Z")
        .expect("right fixture should normalize");
    assert_eq!(left.content_hash, right.content_hash);
    assert_eq!(left.content_hash.as_str().len(), 64);
    assert!(!left.content_hash.as_str().starts_with("v1:"));
    assert_ne!(
        left.provenance.observed_at_utc,
        right.provenance.observed_at_utc
    );
}

#[test]
fn variant_v1_fingerprint_preserves_presence_states() {
    let missing: RawCatalogCourse =
        serde_json::from_str(r#"{"courseString":"SYN:CAT:001","sections":[]}"#)
            .expect("synthetic missing fixture should decode");
    let null: RawCatalogCourse = serde_json::from_str(
        r#"{"courseString":"SYN:CAT:001","supplementCode":null,"sections":[]}"#,
    )
    .expect("synthetic null fixture should decode");
    let empty: RawCatalogCourse =
        serde_json::from_str(r#"{"courseString":"SYN:CAT:001","supplementCode":"","sections":[]}"#)
            .expect("synthetic empty fixture should decode");
    let fingerprints = [
        variant_fingerprint_v1(&missing).unwrap(),
        variant_fingerprint_v1(&null).unwrap(),
        variant_fingerprint_v1(&empty).unwrap(),
    ];
    assert!(fingerprints.iter().all(|value| value.starts_with("v1:")));
    assert_ne!(fingerprints[0], fingerprints[1]);
    assert_ne!(fingerprints[1], fingerprints[2]);
    assert_eq!(missing.supplement_code, Presence::Missing);
}

#[test]
fn variant_v1_uses_frozen_identity_fields_and_excludes_mutable_content() {
    let baseline: RawCatalogCourse = serde_json::from_str(
        r#"{
          "courseString":"SYN:CAT:001","courseNotes":"SYNTHETIC_DETAIL_A","coreCodes":["SYN_CORE"],
          "sections":[{"index":"10001"}],"openSections":1
        }"#,
    )
    .expect("synthetic baseline should decode");
    let detail_changed: RawCatalogCourse = serde_json::from_str(
        r#"{
          "courseString":"SYN:CAT:001","courseNotes":"SYNTHETIC_DETAIL_B","coreCodes":["SYN_CORE"],
          "sections":[{"index":"10001"}],"openSections":1
        }"#,
    )
    .expect("synthetic detail change should decode");
    let identity_changed: RawCatalogCourse = serde_json::from_str(
        r#"{
          "courseString":"SYN:CAT:001","courseNotes":"SYNTHETIC_DETAIL_A","coreCodes":["SYN_CORE"],
          "offeringUnitCode":"SYN_UNIT_B","sections":[{"index":"10001"}],"openSections":1
        }"#,
    )
    .expect("synthetic identity change should decode");
    let section_only_changed: RawCatalogCourse = serde_json::from_str(
        r#"{
          "courseString":"SYN:CAT:001","courseNotes":"SYNTHETIC_DETAIL_A","coreCodes":["SYN_CORE"],
          "sections":[{"index":"99999"}],"openSections":99
        }"#,
    )
    .expect("synthetic section change should decode");

    assert_eq!(
        variant_fingerprint_v1(&baseline).unwrap(),
        variant_fingerprint_v1(&detail_changed).unwrap()
    );
    assert_ne!(
        variant_fingerprint_v1(&baseline).unwrap(),
        variant_fingerprint_v1(&identity_changed).unwrap()
    );
    assert_eq!(
        variant_fingerprint_v1(&baseline).unwrap(),
        variant_fingerprint_v1(&section_only_changed).unwrap()
    );
}

#[test]
fn catalog_contract_enum_mapping_is_lossless_for_unknown_and_conflict_states() {
    assert_eq!(
        bcsp_contracts::CatalogModality::from(DeliveryModality::Other).wire_name(),
        "OTHER"
    );
    assert_eq!(
        bcsp_contracts::CatalogModality::from(DeliveryModality::UnknownConflict).wire_name(),
        "UNKNOWN_CONFLICT"
    );
    assert_eq!(
        bcsp_contracts::CatalogModality::from(OccurrenceModality::Unknown).wire_name(),
        "UNKNOWN"
    );
    assert_eq!(
        bcsp_contracts::CatalogSynchronicity::from(Synchronicity::Unspecified).wire_name(),
        "UNSPECIFIED"
    );
    assert_eq!(
        bcsp_contracts::CatalogSynchronicity::from(Synchronicity::Mixed).wire_name(),
        "MIXED"
    );
}

#[test]
fn every_frozen_offering_identity_field_participates_in_variant_v1() {
    let baseline = json!({
        "campusCode": "SYN",
        "campusLocations": [{"code": "SYN"}],
        "coreCodes": ["SYN_CORE"],
        "courseDescription": "SYNTHETIC_DESCRIPTION_A",
        "courseFee": "SYN_FEE_A",
        "courseFeeDescr": "SYNTHETIC_FEE_DESCRIPTION_A",
        "courseNotes": "SYNTHETIC_NOTES_A",
        "courseNumber": "001",
        "courseString": "SYN:CAT:001",
        "credits": 3,
        "creditsObject": {"code": "3"},
        "expandedTitle": "SYNTHETIC_EXPANDED_A",
        "level": "SYN_LEVEL_A",
        "mainCampus": "SYN",
        "offeringUnitCode": "SYN_UNIT_A",
        "offeringUnitTitle": "SYNTHETIC_UNIT_A",
        "preReqNotes": "SYNTHETIC_PREREQ_A",
        "school": {"code": "SYN_SCHOOL_A"},
        "subject": "SYN_SUBJECT_A",
        "subjectDescription": "SYNTHETIC_SUBJECT_A",
        "subjectGroupNotes": "SYNTHETIC_GROUP_A",
        "subjectNotes": "SYNTHETIC_SUBJECT_NOTES_A",
        "supplementCode": "SYN_SUPPLEMENT_A",
        "synopsisUrl": "SYNTHETIC_URL_A",
        "title": "SYNTHETIC_TITLE_A",
        "unitNotes": "SYNTHETIC_UNIT_NOTES_A",
        "futureFilterFact": {"code": "SYN_FUTURE_A"},
        "openSections": 1,
        "sections": []
    });
    let baseline_course: RawCatalogCourse =
        serde_json::from_value(baseline.clone()).expect("synthetic baseline should decode");
    let baseline_fingerprint = variant_fingerprint_v1(&baseline_course).unwrap();
    let changes = [
        ("creditsObject", json!({"code":"4"})),
        ("expandedTitle", json!("SYNTHETIC_EXPANDED_B")),
        ("level", json!("SYN_LEVEL_B")),
        ("offeringUnitCode", json!("SYN_UNIT_B")),
        ("supplementCode", json!("SYN_SUPPLEMENT_B")),
        ("title", json!("SYNTHETIC_TITLE_B")),
    ];
    for (field, replacement) in changes {
        let mut changed = baseline.clone();
        changed
            .as_object_mut()
            .expect("baseline is an object")
            .insert(field.to_owned(), replacement);
        let changed: RawCatalogCourse =
            serde_json::from_value(changed).expect("synthetic mutation should decode");
        assert_ne!(
            baseline_fingerprint,
            variant_fingerprint_v1(&changed).unwrap(),
            "field {field} must participate in variant identity"
        );
    }
}

#[test]
fn non_identity_course_facts_advance_content_without_changing_variant_key() {
    let left = br#"[{
      "campusCode":"SYN_A","campusLocations":[{"code":"SYN_A"}],
      "courseString":"SYN:CAT:001","title":"SYNTHETIC","courseNotes":"DETAIL_A",
      "sections":[{"campusCode":"SYN_A","index":"41001"}]
    }]"#;
    let right = br#"[{
      "campusCode":"SYN_B","campusLocations":[{"code":"SYN_B"}],
      "courseString":"SYN:CAT:001","title":"SYNTHETIC","courseNotes":"DETAIL_B",
      "sections":[{"campusCode":"SYN_B","index":"41001"}]
    }]"#;
    let left = normalize(left, "T2030", "SYN", "2030-01-01T00:00:00Z").unwrap();
    let right = normalize(right, "T2030", "SYN", "2030-01-02T00:00:00Z").unwrap();

    assert_eq!(
        left.groups[0].variants[0].key,
        right.groups[0].variants[0].key
    );
    assert_ne!(
        left.groups[0].variants[0].canonical_hash,
        right.groups[0].variants[0].canonical_hash
    );
    assert_ne!(left.content_hash, right.content_hash);
}

#[test]
fn canonical_facts_retain_all_section_filter_sources_without_projection_loss() {
    let body = br#"[{
      "campusCode":"SYN","courseString":"SYN:CAT:001","coreCodes":[],"preReqNotes":"",
      "sections":[{
        "campusCode":"SYN","index":"40001","comments":["SYN_COMMENT"],
        "examCode":"SYN_EXAM","examCodeText":"SYNTHETIC_EXAM",
        "honorPrograms":[{"code":"SYN_HONOR"}],"majors":[{"code":"SYN_MAJOR"}],
        "minors":[{"code":"SYN_MINOR"}],"openToText":"SYNTHETIC_OPEN_TO",
        "sectionCampusLocations":[{"code":"SYN"}],"sectionEligibility":"SYNTHETIC_ELIGIBILITY",
        "sessionDates":"SYNTHETIC_SESSION","specialPermissionAddCode":"SYN_ADD",
        "specialPermissionDropCode":"SYN_DROP","unitMajors":[{"code":"SYN_UNIT_MAJOR"}],
        "meetingTimes":[],"instructors":[]
      }]
    }]"#;
    let target = normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("synthetic source coverage fixture should normalize");
    let variant = &target.groups[0].variants[0];
    assert_eq!(variant.fields.core_codes, Presence::Value(Vec::new()));
    assert_eq!(
        variant.fields.prerequisite_notes,
        Presence::Value(String::new())
    );
    let facts = target
        .sections()
        .next()
        .unwrap()
        .canonical_facts
        .as_object()
        .unwrap();
    for field in [
        "comments",
        "examCode",
        "examCodeText",
        "honorPrograms",
        "majors",
        "minors",
        "openToText",
        "sectionCampusLocations",
        "sectionEligibility",
        "sessionDates",
        "specialPermissionAddCode",
        "specialPermissionDropCode",
        "unitMajors",
    ] {
        assert!(facts.contains_key(field), "missing canonical field {field}");
    }
}

#[test]
fn real_shape_mode_90_sections_derive_synchronicity_from_time_facts() {
    // Shapes copied from the real Rutgers feed (synthetic identifiers): the
    // generic ONLINE INSTRUCTION(INTERNET) mode 90 carries its synchronicity
    // in the time facts, not in the mode code.
    let body = br#"[{
      "campusCode":"SYN","courseString":"SYN:CAT:002","sections":[
        {"campusCode":"SYN","index":"20001","sectionCourseType":"O","meetingTimes":[
          {"meetingModeCode":"90","meetingDay":"","startTimeMilitary":"","endTimeMilitary":"","baClassHours":"B","campusLocation":"O"}]},
        {"campusCode":"SYN","index":"20002","sectionCourseType":"O","meetingTimes":[
          {"meetingModeCode":"90","meetingDay":"W","startTimeMilitary":"1000","endTimeMilitary":"1120","baClassHours":"","campusLocation":"7"}]},
        {"campusCode":"SYN","index":"20003","sectionCourseType":"H","meetingTimes":[
          {"meetingModeCode":"02","meetingDay":"W","startTimeMilitary":"1000","endTimeMilitary":"1120","baClassHours":"","campusLocation":"7"},
          {"meetingModeCode":"90","meetingDay":"","startTimeMilitary":"","endTimeMilitary":"","baClassHours":"B","campusLocation":"7"}]},
        {"campusCode":"SYN","index":"20004","sectionCourseType":"O","meetingTimes":[
          {"meetingModeCode":"02","meetingDay":"M","startTimeMilitary":"0900","endTimeMilitary":"1020","baClassHours":"","campusLocation":"O"},
          {"meetingModeCode":"90","meetingDay":"","startTimeMilitary":"","endTimeMilitary":"","baClassHours":"B","campusLocation":"O"}]},
        {"campusCode":"SYN","index":"20005","sectionCourseType":"O","meetingTimes":[
          {"meetingModeCode":"90","meetingDay":"","startTimeMilitary":"","endTimeMilitary":"","baClassHours":"B","campusLocation":""}]}
      ]
    }]"#;
    let target = normalize(body, "T2030", "SYN", "2030-01-01T00:00:00Z")
        .expect("real-shape fixture should normalize");
    let sections = target.sections().collect::<Vec<_>>();
    assert_eq!(sections.len(), 5);

    // NB "O" section: online, hours by arrangement -> ASYNC.
    assert_eq!(sections[0].delivery_modality, DeliveryModality::Online);
    assert_eq!(sections[0].synchronicity, Synchronicity::Asynchronous);
    assert_eq!(
        sections[0].occurrences[0].kind,
        OccurrenceKind::ByArrangement
    );
    assert_eq!(
        sections[0].occurrences[0].normalization_reason,
        "ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS"
    );

    // Newark physical location code on a mode-90 row: the WHERE conflict is
    // preserved, the WHEN is still derived from the scheduled time.
    assert_eq!(
        sections[1].occurrences[0].modality,
        OccurrenceModality::UnknownConflict
    );
    assert_eq!(
        sections[1].occurrences[0].normalization_reason,
        "MODE_LOCATION_CONFLICT"
    );
    assert_eq!(
        sections[1].occurrences[0].synchronicity,
        Synchronicity::Synchronous
    );
    // The frozen section oracle already counts the row as remote evidence
    // for an "O" section; only the occurrence records the conflict.
    assert_eq!(sections[1].delivery_modality, DeliveryModality::Online);
    assert_eq!(sections[1].synchronicity, Synchronicity::Synchronous);

    // Scheduled lecture plus online-by-arrangement component: both members
    // are reliable and differ -> MIXED (hybrid and online sections alike).
    assert_eq!(
        sections[2]
            .occurrences
            .iter()
            .map(|occurrence| occurrence.synchronicity)
            .collect::<Vec<_>>(),
        vec![Synchronicity::Synchronous, Synchronicity::Asynchronous]
    );
    assert_eq!(sections[2].delivery_modality, DeliveryModality::Hybrid);
    assert_eq!(sections[2].synchronicity, Synchronicity::Mixed);
    assert_eq!(sections[3].synchronicity, Synchronicity::Mixed);

    // Empty campusLocation with "B": not the official by-arrangement tuple
    // (kind stays UNSPECIFIED) but the synchronicity is still evident.
    assert_eq!(sections[4].occurrences[0].kind, OccurrenceKind::Unspecified);
    assert_eq!(
        sections[4].occurrences[0].modality,
        OccurrenceModality::Online
    );
    assert_eq!(
        sections[4].occurrences[0].normalization_reason,
        "ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS"
    );
    assert_eq!(sections[4].synchronicity, Synchronicity::Asynchronous);
}
