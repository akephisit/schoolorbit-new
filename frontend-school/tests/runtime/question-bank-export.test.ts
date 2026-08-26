import assert from 'node:assert/strict';
import test from 'node:test';

import { loadQuestionBankExportData } from '../../src/lib/question-bank/export-data.ts';

test('question-bank export loads 200 ordered questions with one cancellable request', async () => {
	const questionIds = Array.from({ length: 200 }, (_, index) => `question-${200 - index}`);
	const controller = new AbortController();
	const calls: Array<{ questionIds: string[]; signal?: AbortSignal }> = [];
	const deps = {
		async exportQuestionBankData(ids: string[], options?: { signal?: AbortSignal }) {
			calls.push({ questionIds: [...ids], signal: options?.signal });
			return ids.map((id) => ({ id }));
		}
	};

	const details = await loadQuestionBankExportData(deps, questionIds, controller.signal);

	assert.equal(calls.length, 1);
	assert.deepEqual(calls[0].questionIds, questionIds);
	assert.equal(calls[0].signal, controller.signal);
	assert.deepEqual(
		details.map((detail) => detail.id),
		questionIds
	);
});
