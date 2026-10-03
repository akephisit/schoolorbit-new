import assert from 'node:assert/strict';
import test from 'node:test';
import {
	saveStaffCreateDraft,
	readStaffCreateDraft,
	clearStaffCreateDraft
} from '../../src/lib/forms/staff-create-draft.ts';
function storage() {
	const values = new Map();
	return {
		values,
		getItem: (key) => values.get(key) ?? null,
		setItem: (key, value) => values.set(key, value),
		removeItem: (key) => values.delete(key)
	};
}
const owner = {
	origin: 'https://synthetic.schoolorbit.invalid',
	userId: '55000000-0000-4000-8000-000000000001'
};
test('draft persists allowed fields and excludes credentials and national ID', () => {
	const store = storage();
	saveStaffCreateDraft(
		store,
		owner,
		{
			first_name: 'ร่าง',
			last_name: 'ทดสอบ',
			password: 'synthetic-private-password',
			confirmPassword: 'synthetic-private-password',
			national_id: 'synthetic-private-identifier'
		},
		100
	);
	const encoded = [...store.values.values()].join('');
	assert.ok(!encoded.includes('synthetic-private-password'));
	assert.ok(!encoded.includes('synthetic-private-identifier'));
	assert.equal(readStaffCreateDraft(store, owner, 101).draft?.first_name, 'ร่าง');
});
test('draft is tenant/user scoped, expires, and removes ownerless legacy storage', () => {
	const store = storage();
	store.setItem('staff-create-draft', '{"first_name":"legacy-owner-unknown"}');
	saveStaffCreateDraft(store, owner, { first_name: 'เจ้าของ' }, 100);
	assert.equal(store.getItem('staff-create-draft'), null);
	assert.equal(
		readStaffCreateDraft(store, { ...owner, origin: 'https://another.schoolorbit.invalid' }, 101)
			.draft,
		null
	);
	assert.equal(
		readStaffCreateDraft(store, { ...owner, userId: '55000000-0000-4000-8000-000000000002' }, 101)
			.draft,
		null
	);
	assert.equal(readStaffCreateDraft(store, owner, 100 + 31 * 60 * 1000).draft, null);
	assert.equal(store.values.size, 0);
});
test('invalid storage is removed and a completed draft clears only its owner', () => {
	const store = storage();
	saveStaffCreateDraft(store, owner, { first_name: 'เจ้าของ' }, 100);
	clearStaffCreateDraft(store, owner);
	assert.equal(store.values.size, 0);
	const another = { ...owner, origin: 'https://another.schoolorbit.invalid' };
	saveStaffCreateDraft(store, another, {}, 100);
	saveStaffCreateDraft(store, owner, {}, 100);
	clearStaffCreateDraft(store, owner);
	assert.ok(readStaffCreateDraft(store, another, 101).draft);
	const key = [...store.values.keys()][0];
	store.setItem(key, 'invalid-json');
	assert.equal(readStaffCreateDraft(store, another, 101).draft, null);
	assert.equal(store.values.size, 0);
});

test('canonical v3 drafts retain text and do not renew imported expiry', () => {
	const store = storage();
	saveStaffCreateDraft(
		store,
		owner,
		{ personnel: { major: 'คณิตศาสตร์', university: 'สถาบันทดสอบ' } },
		100
	);
	assert.equal(readStaffCreateDraft(store, owner, 101).draft.personnel.major, 'คณิตศาสตร์');
	assert.ok([...store.values.keys()].every((key) => key.startsWith('staff-create-draft:v3:')));
});
const oldKey = `staff-create-draft:v2:${owner.origin}:${owner.userId}`;
const referenceId = '55000000-0000-4000-8000-000000000090';
function oldDraft(store, summaryId = referenceId) {
	store.setItem(
		oldKey,
		JSON.stringify({
			version: 2,
			expiresAt: 1000,
			fields: {
				first_name: 'ร่างเดิม',
				personnel: { major_id: referenceId, university_id: null },
				personnel_references: {
					job_position: null,
					major: { id: summaryId, code: 'fixture', name: 'คณิตศาสตร์', isActive: false },
					university: null
				}
			}
		})
	);
}
test('v2 import requires exact matching IDs and removes source only after preservation', () => {
	const store = storage();
	oldDraft(store);
	assert.equal(readStaffCreateDraft(store, owner, 101).draft.personnel.major, 'คณิตศาสตร์');
	assert.equal(store.getItem(oldKey), null);
	const envelope = JSON.parse([...store.values.values()][0]);
	assert.equal(envelope.version, 3);
	assert.equal(envelope.expiresAt, 1000);
	assert.equal(readStaffCreateDraft(store, owner, 1000).draft, null);
});
test('v2 mismatch refuses the complete import visibly and retains source until expiry', () => {
	const store = storage();
	oldDraft(store, '55000000-0000-4000-8000-000000000091');
	assert.deepEqual(readStaffCreateDraft(store, owner, 101), { draft: null, migrationFailed: true });
	assert.ok(store.getItem(oldKey));
	assert.deepEqual(readStaffCreateDraft(store, owner, 1000), {
		draft: null,
		migrationFailed: false
	});
	assert.equal(store.getItem(oldKey), null);
});
