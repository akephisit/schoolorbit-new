CREATE TABLE academic_opening_policy (
    id SMALLINT PRIMARY KEY CHECK (id = 1),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    require_homeroom_placements BOOLEAN NOT NULL DEFAULT false,
    require_published_offerings BOOLEAN NOT NULL DEFAULT false,
    require_published_timetable BOOLEAN NOT NULL DEFAULT false,
    updated_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO academic_opening_policy (id) VALUES (1);
