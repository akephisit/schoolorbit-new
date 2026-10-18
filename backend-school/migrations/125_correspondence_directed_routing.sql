-- Fixed-template processes and directed graphs are distinct supported modes.
-- Existing rows, template versions and immutable evidence are untouched.
ALTER TABLE process_instances ALTER COLUMN template_id DROP NOT NULL;
ALTER TABLE process_instances ALTER COLUMN template_version DROP NOT NULL;
ALTER TABLE process_instances ADD CONSTRAINT process_template_reference_pair
 CHECK ((template_id IS NULL) = (template_version IS NULL));
CREATE TABLE correspondence_routing_settings (
 register text PRIMARY KEY CHECK (register IN ('incoming','outgoing','internal')),
 revision bigint NOT NULL CHECK (revision > 0),
 definition jsonb NOT NULL,
 updated_by uuid NOT NULL REFERENCES users(id),
 updated_at timestamptz NOT NULL DEFAULT now()
);
