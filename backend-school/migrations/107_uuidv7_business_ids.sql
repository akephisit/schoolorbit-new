-- Change automatic business identity defaults, never existing values or foreign keys.
-- The exact v4 default predicate excludes deterministic and custom generators.
DO $$
DECLARE target RECORD;
BEGIN
    FOR target IN
        SELECT n.nspname, c.relname, a.attname
        FROM pg_attribute a
        JOIN pg_class c ON c.oid = a.attrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
        WHERE n.nspname = current_schema() AND c.relkind IN ('r', 'p')
          AND a.attname = 'id' AND a.atttypid = 'uuid'::regtype
          AND pg_get_expr(d.adbin, d.adrelid) ~ '^(public\.|pg_catalog\.)?(gen_random_uuid|uuid_generate_v4)\(\)$'
          AND c.relname <> 'auth_sessions'
    LOOP
        EXECUTE format('ALTER TABLE %I.%I ALTER COLUMN %I SET DEFAULT uuidv7()', target.nspname, target.relname, target.attname);
    END LOOP;
END $$;
