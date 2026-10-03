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
	assert.equal(readStaffCreateDraft(store, owner, 101)?.first_name, 'ร่าง');
});
test('draft is tenant/user scoped, expires, and removes ownerless legacy storage', () => {
	const store = storage();
	store.setItem('staff-create-draft', '{"first_name":"legacy-owner-unknown"}');
	saveStaffCreateDraft(store, owner, { first_name: 'เจ้าของ' }, 100);
	assert.equal(store.getItem('staff-create-draft'), null);
	assert.equal(
		readStaffCreateDraft(store, { ...owner, origin: 'https://another.schoolorbit.invalid' }, 101),
		null
	);
	assert.equal(
		readStaffCreateDraft(store, { ...owner, userId: '55000000-0000-4000-8000-000000000002' }, 101),
		null
	);
	assert.equal(readStaffCreateDraft(store, owner, 100 + 31 * 60 * 1000), null);
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
	assert.ok(readStaffCreateDraft(store, another, 101));
	const key = [...store.values.keys()][0];
	store.setItem(key, 'invalid-json');
	assert.equal(readStaffCreateDraft(store, another, 101), null);
	assert.equal(store.values.size, 0);
});

test('canonical v4 drafts retain text without renewing expiry', () => {
	const store = storage();
	saveStaffCreateDraft(
		store,
		owner,
		{ personnel: { major: 'คณิตศาสตร์', university: 'สถาบันทดสอบ' } },
		100
	);
	assert.equal(readStaffCreateDraft(store, owner, 101).personnel.major, 'คณิตศาสตร์');
	assert.ok([...store.values.keys()].every((key) => key.startsWith('staff-create-draft:v4:')));
});

const oldKey = `staff-create-draft:v2:${owner.origin}:${owner.userId}`;
test('obsolete owner-qualified drafts are removed without reading or importing them', () => {
	const store = storage();
	store.setItem(
		oldKey,
		JSON.stringify({ version: 2, expiresAt: 1000, fields: { first_name: 'obsolete' } })
	);
	store.setItem('staff-create-draft', 'obsolete-ownerless');
	const currentRead = store.getItem;
	store.getItem = (key) => {
		assert.notEqual(key, oldKey, 'obsolete draft contents must never be read');
		assert.notEqual(key, 'staff-create-draft', 'ownerless contents must never be read');
		return currentRead(key);
	};
	assert.equal(readStaffCreateDraft(store, owner, 101), null);
	assert.equal(store.values.size, 0);
	saveStaffCreateDraft(store, owner, { first_name: 'current' }, 100);
	store.setItem(oldKey, 'obsolete-unparseable');
	assert.equal(readStaffCreateDraft(store, owner, 101).first_name, 'current');
	assert.equal(store.values.size, 1);
});
test('canonical draft rejects retired education IDs and discards invalid v3 storage', () => {
	const store = storage();
	assert.throws(() =>
		saveStaffCreateDraft(
			store,
			owner,
			{ personnel: { major_id: '55000000-0000-4000-8000-000000000090' } },
			100
		)
	);
	assert.equal(store.values.size, 0);
	const currentKey = `staff-create-draft:v3:${owner.origin}:${owner.userId}`;
	store.setItem(
		currentKey,
		JSON.stringify({
			version: 3,
			expiresAt: 1000,
			fields: { personnel: { university_id: '55000000-0000-4000-8000-000000000090' } }
		})
	);
	assert.equal(readStaffCreateDraft(store, owner, 101), null);
	assert.equal(store.values.size, 0);
});
test('reading a canonical draft retains its original expiry and refuses unbounded lifetimes', () => {
	const store = storage();
	saveStaffCreateDraft(store, owner, { first_name: 'current' }, 100);
	const currentKey = [...store.values.keys()][0];
	const before = store.getItem(currentKey);
	assert.equal(readStaffCreateDraft(store, owner, 101).first_name, 'current');
	assert.equal(store.getItem(currentKey), before);
	store.setItem(
		currentKey,
		JSON.stringify({
			version: 4,
			expiresAt: 31 * 60 * 1000,
			fields: { first_name: 'invalid-future' }
		})
	);
	assert.equal(readStaffCreateDraft(store, owner, 101), null);
	assert.equal(store.values.size, 0);
});

test('invalid new draft fields preserve the previous canonical draft before storage cleanup', () => {
	const store = storage();
	saveStaffCreateDraft(store, owner, { personnel: { major: 'คณิตศาสตร์' } }, 100);
	const currentKey = [...store.values.keys()][0];
	const before = store.getItem(currentKey);
	store.setItem(oldKey, 'obsolete-private-input');
	assert.throws(() =>
		saveStaffCreateDraft(
			store,
			owner,
			{ personnel: { university_id: '55000000-0000-4000-8000-000000000090' } },
			101
		)
	);
	assert.equal(store.getItem(currentKey), before);
	assert.equal(store.getItem(oldKey), 'obsolete-private-input');
	assert.equal(readStaffCreateDraft(store, owner, 102).personnel.major, 'คณิตศาสตร์');
	assert.equal(store.getItem(oldKey), null);
});

const legacyKey = `staff-create-draft:v3:${owner.origin}:${owner.userId}`;
const currentKey = `staff-create-draft:v4:${owner.origin}:${owner.userId}`;
test('retired v3 drafts are discarded unread without conversion writes', () => {
	const store = storage();
	const another = `${legacyKey}:another-owner`;
	store.setItem(
		legacyKey,
		JSON.stringify({
			version: 3,
			expiresAt: 1000,
			fields: { first_name: 'obsolete', personnel: { academic_rank: 'proficient' } }
		})
	);
	store.setItem(another, 'another-owner');
	const read = store.getItem;
	store.getItem = (key) => {
		assert.notEqual(key, legacyKey);
		return read(key);
	};
	store.setItem = () => {
		throw new Error('read must not write a converted draft');
	};
	assert.equal(readStaffCreateDraft(store, owner, 101), null);
	assert.equal(read(currentKey), null);
	assert.equal(read(legacyKey), null);
	assert.equal(read(another), 'another-owner');
});
test('canonical career survives storage failure and rejects retired career fields', () => {
	const store = storage();
	const details = { effectiveDate: '', orderDate: '', orderNumber: '', note: '', reference: null };
	const career = {
		personnelType: { value: 'civil_servant', ...details },
		jobPosition: { value: null, ...details },
		academicRank: { value: 'proficient', ...details, effectiveDate: '2022-02-28' }
	};
	saveStaffCreateDraft(store, owner, { personnel: { career } }, 100);
	const before = store.getItem(currentKey);
	assert.deepEqual(readStaffCreateDraft(store, owner, 101).personnel.career, career);
	for (const field of ['academic_rank', 'job_position_id']) {
		assert.throws(() => saveStaffCreateDraft(store, owner, { personnel: { [field]: null } }, 102));
	}
	store.setItem = () => {
		throw new Error('storage unavailable');
	};
	assert.throws(
		() => saveStaffCreateDraft(store, owner, { first_name: 'replacement' }, 103),
		/storage unavailable/
	);
	assert.equal(store.getItem(currentKey), before);
});
