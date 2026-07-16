CREATE INDEX catalog_staging_sections_variant_fk
    ON catalog_staging_sections(observation_id, course_string, fingerprint);

CREATE INDEX catalog_sections_variant_fk
    ON catalog_sections(target_id, course_string, fingerprint);
