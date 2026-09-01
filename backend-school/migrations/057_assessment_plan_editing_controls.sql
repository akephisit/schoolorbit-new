ALTER TABLE academic_assessment_phase_controls
    RENAME COLUMN item_editing_enabled TO plan_editing_enabled;

INSERT INTO academic_assessment_phase_controls (
    academic_term_id,
    academic_year_id,
    phase_code
)
SELECT term.id,
       term.academic_year_id,
       phase.phase_code
FROM academic_terms term
CROSS JOIN (
    VALUES
        ('before_midterm'::text),
        ('midterm'::text),
        ('after_midterm'::text),
        ('final'::text)
) AS phase(phase_code)
ON CONFLICT (academic_term_id, phase_code) DO NOTHING;
