import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const root = new URL('../../../', import.meta.url);
const read = (path) => readFile(new URL(path, root), 'utf8');

test('score route owns independent visible regions and uses a minimal score-room roster', async () => {
	const [route, page, api, router, handler, service] = await Promise.all([
		read('frontend-school/src/routes/(app)/staff/academic/admission/[id]/scores/+page.ts'),
		read('frontend-school/src/routes/(app)/staff/academic/admission/[id]/scores/+page.svelte'),
		read('frontend-school/src/lib/api/admission.ts'),
		read('backend-school/src/modules/admission.rs'),
		read('backend-school/src/modules/admission/handlers/scores.rs'),
		read('backend-school/crates/school-admission/src/services/score_service.rs')
	]);
	for (const readName of [
		'getRound',
		'listTracks',
		'listSubjects',
		'getAllScores',
		'getScoreRoomRoster'
	]) {
		assert.match(route, new RegExp(`${readName}\\(`));
	}
	assert.match(route, /requestFetch: fetch/);
	assert.doesNotMatch(route, /getExamSeats\(/);
	assert.doesNotMatch(route, /listApplications\(/);
	assert.doesNotMatch(page, /onMount\(loadAll\)/);
	assert.doesNotMatch(page, /[Hh]ref="\/staff\/academic\/admission\/\{id\}/);
	assert.match(api, /function getScoreRoomRoster\(/);
	assert.match(router, /\/rounds\/\{id\}\/score-room-roster/);
	assert.match(handler, /pub async fn get_score_room_roster[\s\S]*?ADMISSION_SCORES_ALL/);
	assert.match(
		service,
		/pub async fn get_score_room_roster[\s\S]*?admission_exam_seat_assignments/
	);
	assert.doesNotMatch(
		service.match(/pub async fn get_score_room_roster[\s\S]*?\n}\n/)?.[0] ?? '',
		/national_id|decrypt_required/
	);
});
