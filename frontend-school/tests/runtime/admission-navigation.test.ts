import assert from 'node:assert/strict';
import test from 'node:test';
import { admissionWorkflowBackHref } from '../../src/lib/navigation/admission.ts';

test('admission action-only return keeps year context without entering applicant administration', () => {
	assert.equal(admissionWorkflowBackHref('round', 'year', false), '/staff?academicYearId=year');
	assert.equal(admissionWorkflowBackHref('round', null, false), '/staff');
	assert.equal(admissionWorkflowBackHref('round', 'year', true), '/staff/academic/admission/round');
});
