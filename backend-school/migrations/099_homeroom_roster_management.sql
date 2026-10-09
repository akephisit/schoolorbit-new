-- Cancel a planned placement without discarding its identity or audit history.
ALTER TABLE homeroom_placements DROP CONSTRAINT homeroom_placements_status_check;
ALTER TABLE homeroom_placements ADD CONSTRAINT homeroom_placements_status_check
    CHECK (status IN ('planned', 'current', 'ended', 'cancelled'));

-- Thai dictionary ordering handles leading vowels and combining marks.
CREATE COLLATION schoolorbit_thai (provider = icu, locale = 'th', deterministic = true);
