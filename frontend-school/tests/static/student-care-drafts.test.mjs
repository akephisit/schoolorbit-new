import assert from 'node:assert/strict';
import test from 'node:test';
import {
	careDefinitionSchema,
	parseCareDefinition,
	emptyCareDefinition,
	setCareAnswer,
	responseError,
	careResponsePayload
} from '../../src/lib/features/student-care/drafts.ts';
const definition = () => ({
	...emptyCareDefinition(),
	name: 'Synthetic school fixture',
	variant: 'Synthetic only',
	source: 'Synthetic nonclinical fixture',
	licenseNote: 'Synthetic fixture author permission',
	mode: 'digital',
	digitalRightsConfirmed: true,
	scales: [
		{
			key: 's',
			label: 'Synthetic scale',
			sourceScaleKeys: [],
			bands: [{ key: 'all', label: 'Synthetic result', minimum: 0, maximum: 2, urgent: false }]
		}
	],
	questions: [
		{
			key: 'q',
			text: 'Synthetic choice',
			scaleKey: 's',
			reverse: false,
			options: [
				{ key: 'a', label: 'Synthetic A', points: 0, urgent: false },
				{ key: 'b', label: 'Synthetic B', points: 2, urgent: true }
			]
		}
	]
});
const detail = (d = definition()) => ({
	assignment: { id: 'assignment', rowVersion: 7 },
	instrument: { id: 'version', definition: d },
	answers: [],
	manualScores: []
});
test('library imports complete validated definitions and rejects instrument status fields', () => {
	const d = definition();
	assert.deepEqual(parseCareDefinition(JSON.stringify(d)), d);
	for (const json of [JSON.stringify({ ...d, status: 'ready' }), '[]', '{'])
		assert.throws(() => parseCareDefinition(json));
});
test('digital rights and manual versus digital content boundaries are explicit', () => {
	assert.equal(
		careDefinitionSchema.safeParse({ ...definition(), digitalRightsConfirmed: false }).success,
		false
	);
	assert.equal(
		careDefinitionSchema.safeParse({ ...definition(), mode: 'manual_result' }).success,
		false
	);
	assert.equal(
		careDefinitionSchema.safeParse({
			...definition(),
			mode: 'manual_result',
			digitalRightsConfirmed: false,
			questions: []
		}).success,
		true
	);
});
test('definition validation rejects duplicate keys and uncovered score ranges', () => {
	const d = definition();
	assert.equal(
		careDefinitionSchema.safeParse({ ...d, questions: [...d.questions, ...d.questions] }).success,
		false
	);
	assert.equal(
		careDefinitionSchema.safeParse({
			...d,
			scales: [{ ...d.scales[0], bands: [{ ...d.scales[0].bands[0], maximum: 1 }] }]
		}).success,
		false
	);
});
test('aggregate scales accept base references but reject cycles and unknown references', () => {
	const d = definition();
	const aggregate = {
		key: 'total',
		label: 'Synthetic total',
		sourceScaleKeys: ['s'],
		bands: d.scales[0].bands
	};
	assert.equal(
		careDefinitionSchema.safeParse({ ...d, scales: [...d.scales, aggregate] }).success,
		true
	);
	for (const sourceScaleKeys of [['total'], ['missing'], ['s', 's']])
		assert.equal(
			careDefinitionSchema.safeParse({
				...d,
				scales: [...d.scales, { ...aggregate, sourceScaleKeys }]
			}).success,
			false
		);
});
test('changing or clearing a draft answer preserves unrelated answers and never fills zero', () => {
	const original = [
		{ questionKey: 'q', optionKey: 'a' },
		{ questionKey: 'other', optionKey: 'b' }
	];
	assert.deepEqual(setCareAnswer(original, 'q', 'b'), [
		{ questionKey: 'other', optionKey: 'b' },
		{ questionKey: 'q', optionKey: 'b' }
	]);
	assert.deepEqual(setCareAnswer(original, 'q', ''), [{ questionKey: 'other', optionKey: 'b' }]);
	assert.equal(original[0].optionKey, 'a');
});
test('incomplete responses remain drafts; final refuses missing or invalid choices', () => {
	const d = detail();
	assert.equal(responseError(d, [], [], false), null);
	assert.match(responseError(d, [], [], true), /ครบ/);
	assert.match(responseError(d, [{ questionKey: 'q', optionKey: 'missing' }], [], false), /ไม่ตรง/);
	assert.equal(responseError(d, [{ questionKey: 'q', optionKey: 'a' }], [], true), null);
});
test('manual drafts preserve missing values and final requires valid base scales only', () => {
	const d = definition();
	d.mode = 'manual_result';
	d.questions = [];
	d.scales.push({
		key: 'total',
		label: 'Synthetic total',
		sourceScaleKeys: ['s'],
		bands: d.scales[0].bands
	});
	const record = detail(d);
	assert.equal(responseError(record, [], [], false), null);
	assert.match(responseError(record, [], [], true), /ทุกมาตรวัด/);
	assert.equal(responseError(record, [], [{ scaleKey: 's', score: 0 }], true), null);
	assert.match(responseError(record, [], [{ scaleKey: 's', score: 3 }], true), /ช่วงคะแนน/);
	assert.match(responseError(record, [], [{ scaleKey: 's', score: NaN }], false), /ตรวจสอบ/);
});
test('payload pins saved revision, purpose and mode-specific content', () => {
	const record = detail();
	const answers = [{ questionKey: 'q', optionKey: 'b' }];
	assert.deepEqual(
		careResponsePayload(record, answers, [{ scaleKey: 's', score: 1 }], '   ', true, 'request'),
		{
			requestId: 'request',
			rowVersion: 7,
			answers,
			manualScores: [],
			manualNote: null,
			acknowledgePurpose: true
		}
	);
	const d = definition();
	d.mode = 'manual_result';
	d.questions = [];
	const payload = careResponsePayload(
		detail(d),
		answers,
		[{ scaleKey: 's', score: 0 }],
		' reference ',
		false,
		'manual'
	);
	assert.deepEqual(payload.answers, []);
	assert.equal(payload.manualNote, 'reference');
	assert.deepEqual(payload.manualScores, [{ scaleKey: 's', score: 0 }]);
});

test('mutation receipt identity survives unchanged retries and changes with content or operation', async () => {
	const { CareMutationIdentity } = await import('../../src/lib/features/student-care/mutation.ts');
	const identity = new CareMutationIdentity();
	const first = identity.prepare(
		{ requestId: 'ignored-1', rowVersion: 1, note: 'sensitive draft' },
		'draft'
	);
	assert.equal(
		identity.prepare({ requestId: 'ignored-2', rowVersion: 1, note: 'sensitive draft' }, 'draft')
			.requestId,
		first.requestId
	);
	assert.notEqual(
		identity.prepare({ requestId: 'ignored-3', rowVersion: 1, note: 'changed draft' }, 'draft')
			.requestId,
		first.requestId
	);
	const draft = identity.prepare({ requestId: 'ignored-4', rowVersion: 1, note: 'same' }, 'draft');
	assert.notEqual(
		identity.prepare({ requestId: 'ignored-5', rowVersion: 1, note: 'same' }, 'submit').requestId,
		draft.requestId
	);
	identity.clear();
	assert.notEqual(
		identity.prepare({ requestId: 'ignored-6', rowVersion: 1, note: 'sensitive draft' }, 'draft')
			.requestId,
		first.requestId
	);
});
