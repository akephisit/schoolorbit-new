import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import test from 'node:test';

test('restore reconciliation normalizes only equivalent casts and boolean association', () => {
  const result = execFileSync('python3', ['-B', '-c', `
import importlib.util
s=importlib.util.spec_from_file_location('transfer','scripts/postgres_data_transfer.py')
m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
same=[
 ("CHECK (((a > 0) AND (a < 9)) AND (b IS NULL))", "CHECK ((a > 0) AND (a < 9) AND (b IS NULL))"),
 ("CHECK (a = ANY ((ARRAY['x'::character varying, 'y'::character varying])::text[]))", "CHECK (a = ANY (ARRAY[('x'::character varying)::text, ('y'::character varying)::text]))"),
 ("CHECK ((a OR b) OR c)", "CHECK (a OR (b OR c))"),
]
for left,right in same: assert m.normalize_constraint(left)==m.normalize_constraint(right)
different=[
 ("CHECK (a AND (b OR c))", "CHECK ((a AND b) OR c)"),
 ("CHECK (a > 0)", "CHECK (a >= 0)"),
 ("CHECK (a AND b)", "CHECK (b AND a)"),
 ("CHECK (a = 'AND (OR)')", "CHECK (a = 'OR (AND)')"),
 ("CHECK (a = 'long'::character varying(2))", "CHECK (a = 'long'::text)"),
]
for left,right in different: assert m.normalize_constraint(left)!=m.normalize_constraint(right)
assert m.normalize_constraint('NOT NULL id')=='NOT NULL id'
print('constraint-preservation-cases-passed')
`], { encoding: 'utf8' });
  assert.equal(result.trim(), 'constraint-preservation-cases-passed');
});
