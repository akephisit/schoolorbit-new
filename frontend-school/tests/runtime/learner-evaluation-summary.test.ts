import assert from 'node:assert/strict';
import test from 'node:test';

import {
	learnerEvaluationLockBlockerLabel,
	learnerEvaluationQualityLabel,
	presentLearnerEvaluationDomain
} from '../../src/lib/academic/learner-evaluation/presentation.ts';

test('learner evaluation quality uses the exact Thai labels for levels zero through three', () => {
	assert.equal(learnerEvaluationQualityLabel(3), 'ดีเยี่ยม');
	assert.equal(learnerEvaluationQualityLabel(2), 'ดี');
	assert.equal(learnerEvaluationQualityLabel(1), 'ผ่าน');
	assert.equal(learnerEvaluationQualityLabel(0), 'ไม่ผ่าน');
	assert.equal(learnerEvaluationQualityLabel(null), 'ยังสรุปไม่ได้');
});

test('learner lock blockers are actionable Thai messages', () => {
	assert.equal(
		learnerEvaluationLockBlockerLabel('missing_responses'),
		'ยังประเมินนักเรียนไม่ครบทุกหัวข้อ'
	);
	assert.equal(
		learnerEvaluationLockBlockerLabel('stale_group_confirmation'),
		'ข้อมูลเปลี่ยนหลังยืนยัน กรุณายืนยันกลุ่มเรียนใหม่'
	);
});

test('a provisional learner summary keeps every missing subject reason visible', () => {
	const presentation = presentLearnerEvaluationDomain({
		domain: 'desirable_characteristic',
		complete: false,
		average: { decimal: '2.25', numerator: '9', denominator: '4' },
		qualityLevel: 2,
		catalogCriteria: [],
		subjects: [],
		missingSubjects: [
			{ subjectId: 'subject-math', reason: 'ค21101 ยังไม่ได้ล็อกผล' },
			{ subjectId: 'subject-thai', reason: 'ท21101 ยังยืนยันไม่ครบทุกห้อง' }
		]
	});

	assert.equal(presentation.status, 'provisional');
	assert.equal(presentation.statusLabel, 'ผลชั่วคราว');
	assert.deepEqual(presentation.reasons, [
		'ค21101 ยังไม่ได้ล็อกผล',
		'ท21101 ยังยืนยันไม่ครบทุกห้อง'
	]);
	assert.equal(presentation.qualityLabel, 'ดี');
});

test('a complete learner summary has no provisional reasons', () => {
	const presentation = presentLearnerEvaluationDomain({
		domain: 'reading_thinking_writing',
		complete: true,
		average: { decimal: '2.75', numerator: '11', denominator: '4' },
		qualityLevel: 3,
		catalogCriteria: [],
		subjects: [],
		missingSubjects: []
	});

	assert.equal(presentation.status, 'complete');
	assert.equal(presentation.statusLabel, 'สรุปครบแล้ว');
	assert.deepEqual(presentation.reasons, []);
	assert.equal(presentation.averageLabel, '2.75');
});

test('known provisional summary reasons use subject labels and Thai remediation', () => {
	const presentation = presentLearnerEvaluationDomain(
		{
			complete: false,
			missingSubjects: [{ subjectId: 'subject-math', reason: 'subject_domain_not_locked' }]
		},
		{ 'subject-math': 'ค21101 · คณิตศาสตร์พื้นฐาน' }
	);

	assert.deepEqual(presentation.reasons, ['ค21101 · คณิตศาสตร์พื้นฐาน · ยังไม่ได้ล็อกผลด้านนี้']);
});
