import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const root = new URL('../../../', import.meta.url);
const read = (path) => readFile(new URL(path, root), 'utf8');

test('selection route owns independent context and dependent visible rankings', async () => {
	const [route, page, api, service] = await Promise.all([
		read('frontend-school/src/routes/(app)/staff/academic/admission/[id]/selections/+page.ts'),
		read('frontend-school/src/routes/(app)/staff/academic/admission/[id]/selections/+page.svelte'),
		read('frontend-school/src/lib/api/admission.ts'),
		read('backend-school/crates/school-admission/src/services/selection_service.rs')
	]);
	for (const readName of [
		'getRound',
		'listTracks',
		'listSubjects',
		'getTrackRanking',
		'getGlobalRanking',
		'getRoomsForRound'
	]) {
		assert.match(route, new RegExp(`${readName}\\(`));
	}
	assert.match(route, /captureRouteLoad/);
	assert.match(route, /requestFetch: fetch/);
	assert.doesNotMatch(page, /onMount\(loadBase\)/);
	assert.match(page, /\$effect\.pre/);
	assert.match(page, /LatestRequest/);
	assert.match(page, /PageSkeleton/);
	assert.match(page, /PageState/);
	assert.doesNotMatch(page, /backHref="\/staff\/academic\/admission\/\{id\}"/);
	for (const readName of ['getTrackRanking', 'getGlobalRanking', 'getRoomsForRound']) {
		const declaration =
			api.match(new RegExp(`export async function ${readName}\\([\\s\\S]*?\\n}`))?.[0] ?? '';
		assert.match(declaration, /ApiRequestOptions/);
		assert.match(declaration, /apiClient\.get<[\s\S]*?,\s*options\s*\)/);
	}
	const detailed = service.match(/pub struct RankRowDetailed \{[\s\S]*?\n\}/)?.[0] ?? '';
	assert.doesNotMatch(detailed, /national_id/);
	for (const responseName of ['TrackRankingEntry', 'GlobalRankingEntry']) {
		const response =
			service.match(new RegExp(`pub struct ${responseName} \\{[\\s\\S]*?\\n\\}`))?.[0] ?? '';
		assert.doesNotMatch(response, /national_id/);
	}
	const trackRead =
		service.split('pub async fn get_track_ranking')[1]?.split('pub async fn assign_rooms')[0] ?? '';
	const globalRead =
		service.split('pub async fn get_global_ranking')[1]?.split('pub async fn get_round_rooms')[0] ??
		'';
	assert.doesNotMatch(trackRead, /unwrap_or_default\(\)/);
	assert.doesNotMatch(globalRead, /unwrap_or_default\(\)/);
	assert.doesNotMatch(trackRead, /aa\.national_id|decrypt_rank_row_detailed/);
	assert.doesNotMatch(globalRead, /aa\.national_id|decrypt_rank_row_detailed/);
	for (const [start, end] of [
		['pub async fn get_round_ranking', 'pub async fn get_track_ranking'],
		['pub async fn assign_rooms(', 'pub async fn reset_all_room_assignments'],
		['pub async fn assign_rooms_global', 'pub async fn get_global_ranking']
	]) {
		const segment = service.split(start)[1]?.split(end)[0] ?? '';
		assert.doesNotMatch(segment, /aa\.national_id|decrypt_rank_row/);
	}
	const roundRead =
		service
			.split('pub async fn get_round_ranking')[1]
			?.split('pub async fn get_track_ranking')[0] ?? '';
	assert.doesNotMatch(roundRead, /for \(track_id, track_name, tiebreak\) in tracks/);
	assert.match(roundRead, /LEFT JOIN admission_applications aa/);
});
