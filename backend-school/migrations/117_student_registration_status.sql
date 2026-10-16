-- Students owns this administrative assertion. It does not control account access,
-- annual enrollment, class placement, attendance membership or academic outcomes.
-- Historical account and annual statuses cannot identify past transfers reliably.
ALTER TABLE student_info
    ADD COLUMN registration_status text NOT NULL DEFAULT 'unrecorded'
        CHECK (registration_status IN (
            'unrecorded', 'pending', 'studying', 'transferred_out', 'withdrawn', 'graduated'
        )),
    ADD COLUMN registration_status_updated_at timestamptz,
    ADD COLUMN registration_status_updated_by uuid REFERENCES users(id) ON DELETE SET NULL;

-- Preserve existing identities and unknown history; future registrations start pending.
ALTER TABLE student_info ALTER COLUMN registration_status SET DEFAULT 'pending';
