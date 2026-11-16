-- Staff control cycle booking with status. Remove only the retired timestamps;
-- retain every other cycle fact and all related supervision evidence unchanged.
SET LOCAL lock_timeout = '30s';
LOCK TABLE supervision_cycles IN ACCESS EXCLUSIVE MODE;

DO $$
DECLARE
    original_count bigint; original_hash text;
    retained_count bigint; retained_hash text;
BEGIN
    SELECT count(*), encode(sha256(convert_to(COALESCE(string_agg(
        encode(sha256(convert_to((to_jsonb(cycle) - 'booking_opens_at' - 'booking_closes_at' - 'starts_at' - 'ends_at')::text,'UTF8')),'hex'),
        '' ORDER BY id),''),'UTF8')),'hex')
    INTO original_count, original_hash FROM supervision_cycles cycle;

    ALTER TABLE supervision_cycles
        DROP CONSTRAINT supervision_cycles_window_check RESTRICT,
        DROP CONSTRAINT supervision_cycles_booking_window_check RESTRICT,
        DROP COLUMN booking_opens_at RESTRICT,
        DROP COLUMN booking_closes_at RESTRICT,
        DROP COLUMN starts_at RESTRICT,
        DROP COLUMN ends_at RESTRICT;

    SELECT count(*), encode(sha256(convert_to(COALESCE(string_agg(
        encode(sha256(convert_to(to_jsonb(cycle)::text,'UTF8')),'hex'),
        '' ORDER BY id),''),'UTF8')),'hex')
    INTO retained_count, retained_hash FROM supervision_cycles cycle;
    IF (retained_count, retained_hash) IS DISTINCT FROM (original_count, original_hash) THEN
        RAISE EXCEPTION 'SUPERVISION_CYCLE159_RECONCILIATION_FAILED' USING ERRCODE='check_violation';
    END IF;
END $$;
