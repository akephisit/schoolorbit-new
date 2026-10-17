-- Student Care owns confidential support independently of Behavior score effects.
-- Definitions are append-only: no instrument lifecycle status is stored.
CREATE TABLE care_instrument_versions (
 id UUID PRIMARY KEY, instrument_id UUID NOT NULL, version INTEGER NOT NULL CHECK(version>0),
 document JSONB NOT NULL, created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 UNIQUE(instrument_id,version), CHECK((document->>'id')::uuid=id),
 CHECK((document->>'instrumentId')::uuid=instrument_id), CHECK((document->>'version')::integer=version)
);
CREATE FUNCTION care_instrument_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'care_instrument_version_immutable'; END $$;
CREATE TRIGGER care_instrument_versions_immutable BEFORE UPDATE OR DELETE ON care_instrument_versions FOR EACH ROW EXECUTE FUNCTION care_instrument_immutable();
CREATE TABLE care_rounds (
 id UUID PRIMARY KEY, academic_term_id UUID NOT NULL REFERENCES academic_terms(id) ON DELETE RESTRICT,
 document JSONB NOT NULL, created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 CHECK((document->>'id')::uuid=id), CHECK((document->>'academicTermId')::uuid=academic_term_id),
 CHECK(document->>'status' IN('open','closed','cancelled')), CHECK((document->>'rowVersion')::bigint>0)
);
CREATE TABLE care_round_versions (
 round_id UUID NOT NULL REFERENCES care_rounds(id) ON DELETE RESTRICT,
 version_id UUID NOT NULL REFERENCES care_instrument_versions(id) ON DELETE RESTRICT,
 PRIMARY KEY(round_id,version_id)
);
CREATE TABLE care_assignments (
 id UUID PRIMARY KEY, round_id UUID NOT NULL REFERENCES care_rounds(id) ON DELETE RESTRICT,
 student_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 respondent_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 instrument_version_id UUID NOT NULL REFERENCES care_instrument_versions(id) ON DELETE RESTRICT,
 created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 reviewer_ids UUID[] NOT NULL CHECK(cardinality(reviewer_ids)>0),
 document JSONB NOT NULL, response_ciphertext TEXT NOT NULL,
 purpose_acknowledged_at TIMESTAMPTZ,
 CHECK((document->>'id')::uuid=id), CHECK((document->>'roundId')::uuid=round_id),
 CHECK((document->>'studentId')::uuid=student_id), CHECK((document->>'respondentId')::uuid=respondent_id),
 CHECK((document->>'instrumentVersionId')::uuid=instrument_version_id),
 CHECK(document->>'status' IN('assigned','in_progress','submitted','cancelled')),
 CHECK((document->>'rowVersion')::bigint>0),
 UNIQUE(round_id,student_id,respondent_id,instrument_version_id),
 FOREIGN KEY(round_id,instrument_version_id) REFERENCES care_round_versions(round_id,version_id) ON DELETE RESTRICT
);
CREATE INDEX care_assignments_creator_idx ON care_assignments(created_by,id);
CREATE INDEX care_assignments_respondent_idx ON care_assignments(respondent_id,id);
CREATE INDEX care_assignments_student_idx ON care_assignments(student_id,id);
CREATE INDEX care_assignments_reviewers_idx ON care_assignments USING gin(reviewer_ids);
CREATE TABLE care_visits (
 id UUID PRIMARY KEY, student_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 visitor_ids UUID[] NOT NULL CHECK(cardinality(visitor_ids)>0), document JSONB NOT NULL,
 record_ciphertext TEXT NOT NULL, CHECK((document->>'id')::uuid=id),
 CHECK((document->>'studentId')::uuid=student_id), CHECK(document->>'status' IN('planned','completed','cancelled')),
 CHECK((document->>'rowVersion')::bigint>0)
);
CREATE INDEX care_visits_scope_idx ON care_visits USING gin(visitor_ids);
CREATE TABLE care_cases (
 id UUID PRIMARY KEY, student_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 assigned_user_ids UUID[] NOT NULL CHECK(cardinality(assigned_user_ids)>0),
 next_follow_up DATE, status TEXT NOT NULL CHECK(status IN('open','monitoring','closed')), row_version BIGINT NOT NULL CHECK(row_version>0),
 source_assignment_id UUID REFERENCES care_assignments(id) ON DELETE RESTRICT,
 source_visit_id UUID REFERENCES care_visits(id) ON DELETE RESTRICT,
 record_ciphertext TEXT NOT NULL
);
CREATE TABLE care_case_events (
 id UUID PRIMARY KEY, case_id UUID NOT NULL REFERENCES care_cases(id) ON DELETE RESTRICT,
 event_ciphertext TEXT NOT NULL
);
CREATE INDEX care_case_events_page_idx ON care_case_events(case_id,id);
CREATE INDEX care_cases_scope_idx ON care_cases USING gin(assigned_user_ids);
CREATE INDEX care_cases_due_idx ON care_cases(next_follow_up,id) WHERE status<>'closed';
CREATE INDEX care_cases_student_idx ON care_cases(student_id,id);
CREATE TABLE care_mutation_receipts (
 actor_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT, request_id UUID NOT NULL,
 operation TEXT NOT NULL, digest TEXT NOT NULL, outcome_ciphertext TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY(actor_id,request_id)
);
-- Audit keeps identities/actions/revisions, never response content or case prose.
CREATE TABLE care_audit (
 id UUID PRIMARY KEY DEFAULT uuidv7(), actor_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 resource_id UUID NOT NULL, action TEXT NOT NULL, revision BIGINT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO permissions(code,module,action,scope,name,description)
SELECT module||'.'||action||'.'||scope,module,action,scope,name,name FROM (VALUES
 ('student_care','read','own','ดูแบบประเมินที่ได้รับมอบหมายของตนเอง'),
 ('student_care','read','assigned','ดูงานดูแลช่วยเหลือที่มอบหมาย'),
 ('student_care','read','school','ดูภาพรวมงานดูแลช่วยเหลือโรงเรียน'),
 ('student_care','manage','assigned','จัดการงานดูแลช่วยเหลือที่มอบหมาย'),
 ('student_care','manage','school','จัดการรอบและงานดูแลช่วยเหลือโรงเรียน'),
 ('student_care_config','manage','school','ตั้งค่ารุ่นแบบประเมินดูแลช่วยเหลือ'),
 ('student_care_sensitive','read','assigned','ดูข้อมูลลับเฉพาะที่มอบหมายอย่างชัดเจน'),
 ('student_care_sensitive','read','school','ดูข้อมูลลับงานดูแลช่วยเหลือโรงเรียน'),
 ('student_care_sensitive','manage','assigned','จัดการข้อมูลลับเฉพาะที่มอบหมายอย่างชัดเจน'),
 ('student_care_sensitive','manage','school','จัดการข้อมูลลับงานดูแลช่วยเหลือโรงเรียน')
) p(module,action,scope,name)
ON CONFLICT(code) DO NOTHING;
-- Routine responder access never implies confidential school access. Existing
-- school administrators configure grants explicitly for confidential reviewers.
INSERT INTO role_permissions(role_id,permission_id)
SELECT r.id,p.id FROM roles r JOIN permissions p ON p.module='student_care' AND p.action='read' AND p.scope='own'
WHERE (r.user_type IN('student','parent') OR r.code='TEACHER') ON CONFLICT DO NOTHING;

INSERT INTO role_permissions(role_id,permission_id)
SELECT r.id,p.id FROM roles r JOIN permissions p ON p.module IN('student_care','student_care_config')
WHERE r.code IN('ADMIN','SUPER_ADMIN') ON CONFLICT DO NOTHING;
