-- Counters share the existing global application-number namespace. Only the
-- canonical eight-digit prefix plus five-digit suffix participates; historical
-- labels and null numbers retain their exact values and identities.
CREATE TABLE admission_application_number_counters (
    prefix TEXT PRIMARY KEY CHECK (prefix ~ '^[0-9]{8}$'),
    last_sequence INTEGER NOT NULL CHECK (last_sequence BETWEEN 0 AND 99999)
);

INSERT INTO admission_application_number_counters(prefix, last_sequence)
SELECT left(application_number,8), max(right(application_number,5)::integer)
FROM admission_applications
WHERE application_number ~ '^[0-9]{13}$'
GROUP BY left(application_number,8);

-- Retain the existing global unique application_number constraint and the
-- (national_id_hash, admission_round_id) unique index as the final authority.
