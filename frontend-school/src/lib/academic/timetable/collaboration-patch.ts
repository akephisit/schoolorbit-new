import type { DraftPatch, TimetableBlockWorkspace } from '../../api/timetable.js';
import { patchTimetableWorkspaceBlocks, setTimetableWorkspaceBlocks } from './board-state.ts';

function mergeBy<T>(existing: readonly T[], changed: readonly T[], key: (value: T) => string): T[] {
	const replacement = new Map(changed.map((value) => [key(value), value]));
	const ids = new Set(existing.map(key));
	return [
		...existing.map((value) => replacement.get(key(value)) ?? value),
		...changed.filter((value) => !ids.has(key(value)))
	];
}

/** Apply only after local pending operations have reconciled their temporary blocks. */
export function applyDraftPatch(
	workspace: TimetableBlockWorkspace,
	patch: DraftPatch
): TimetableBlockWorkspace {
	if (workspace.version.id !== patch.timetableVersionId) return workspace;
	const removed = new Set(patch.removedBlockIds);
	const withoutRemoved = setTimetableWorkspaceBlocks(
		workspace,
		workspace.blocks.filter((block) => !removed.has(block.id))
	);
	const currentById = new Map(withoutRemoved.blocks.map((block) => [block.id, block]));
	const updated = patchTimetableWorkspaceBlocks(
		withoutRemoved,
		patch.upsertedBlocks.filter(
			(block) => (currentById.get(block.id)?.rowVersion ?? 0) <= block.rowVersion
		)
	);
	return {
		...updated,
		version: patch.version ?? workspace.version,
		ordinaryDemands: mergeBy(
			workspace.ordinaryDemands,
			patch.ordinaryDemands,
			(demand) => demand.learningGroupId
		),
		synchronizedDemands: mergeBy(
			workspace.synchronizedDemands,
			patch.synchronizedDemands,
			(demand) => demand.learningOfferingId
		),
		summary: patch.summary,
		sourceIssues: patch.sourceIssues,
		totalDraftBlockCount:
			patch.totalDraftBlockCount === undefined
				? workspace.totalDraftBlockCount
				: patch.totalDraftBlockCount
	};
}
