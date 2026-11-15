-- Current operations no longer accept offering effectivity dates. Keep the
-- original request hashes, identities, outputs and curriculum provenance exact.
-- Deploy the matching consumer under maintenance; recover forward after commit.
SET LOCAL lock_timeout = '30s';
LOCK TABLE learning_delivery_apply_runs, academic_term_preparation_runs IN ACCESS EXCLUSIVE MODE;

DO $$
DECLARE
    apply_count bigint; apply_hash text; preparation_count bigint; preparation_hash text;
    retained_count bigint; retained_hash text;
BEGIN
    SELECT count(*), encode(sha256(convert_to(COALESCE(string_agg(
        encode(sha256(convert_to((to_jsonb(receipt)-'starts_on')::text,'UTF8')),'hex'),
        '' ORDER BY idempotency_key),''),'UTF8')),'hex')
    INTO apply_count, apply_hash FROM learning_delivery_apply_runs receipt;
    SELECT count(*), encode(sha256(convert_to(COALESCE(string_agg(
        encode(sha256(convert_to((to_jsonb(receipt)-'offering_starts_on')::text,'UTF8')),'hex'),
        '' ORDER BY id),''),'UTF8')),'hex')
    INTO preparation_count, preparation_hash FROM academic_term_preparation_runs receipt;

    ALTER TABLE learning_delivery_apply_runs DROP COLUMN starts_on RESTRICT;
    ALTER TABLE academic_term_preparation_runs DROP COLUMN offering_starts_on RESTRICT;

    SELECT count(*), encode(sha256(convert_to(COALESCE(string_agg(
        encode(sha256(convert_to(to_jsonb(receipt)::text,'UTF8')),'hex'),
        '' ORDER BY idempotency_key),''),'UTF8')),'hex')
    INTO retained_count, retained_hash FROM learning_delivery_apply_runs receipt;
    IF (retained_count, retained_hash) IS DISTINCT FROM (apply_count, apply_hash) THEN
        RAISE EXCEPTION 'ACADEMIC_RECEIPT_DATE_CLEANUP_RECONCILIATION_FAILED' USING ERRCODE='check_violation';
    END IF;
    SELECT count(*), encode(sha256(convert_to(COALESCE(string_agg(
        encode(sha256(convert_to(to_jsonb(receipt)::text,'UTF8')),'hex'),
        '' ORDER BY id),''),'UTF8')),'hex')
    INTO retained_count, retained_hash FROM academic_term_preparation_runs receipt;
    IF (retained_count, retained_hash) IS DISTINCT FROM (preparation_count, preparation_hash) THEN
        RAISE EXCEPTION 'ACADEMIC_RECEIPT_DATE_CLEANUP_RECONCILIATION_FAILED' USING ERRCODE='check_violation';
    END IF;
END $$;
