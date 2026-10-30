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

export class InvalidDraftLookupPatch extends Error {
	constructor() {
		super('Timetable lookup order does not match its authorized rows');
		this.name = 'InvalidDraftLookupPatch';
	}
}

function mergeLookup<T>(
	existing: readonly T[],
	upserted: readonly T[],
	removedIds: readonly string[],
	order: readonly string[] | null | undefined,
	key: (value: T) => string
): T[] {
	if (upserted.length === 0 && removedIds.length === 0 && order == null) return [...existing];
	const removed = new Set(removedIds);
	const next = mergeBy(
		existing.filter((value) => !removed.has(key(value))),
		upserted,
		key
	);
	const rows = new Map(next.map((value) => [key(value), value]));
	if (
		!order ||
		rows.size !== next.length ||
		order.length !== rows.size ||
		new Set(order).size !== order.length ||
		order.some((id) => !rows.has(id))
	)
		throw new InvalidDraftLookupPatch();
	return order.map((id) => rows.get(id)!);
}

/** Apply only after local pending operations have reconciled their temporary blocks. */
export function applyDraftPatch(
	workspace: TimetableBlockWorkspace,
	patch: DraftPatch
): TimetableBlockWorkspace {
	if (workspace.version.id !== patch.timetableVersionId) return workspace;
	const lookups = patch.lookupChanges;
	// Validate and assemble every lookup before exposing any part of the new workspace.
	const lookupWorkspace = lookups
		? {
				staff: mergeLookup(
					workspace.staff,
					lookups.upsertedStaff,
					lookups.removedStaffIds,
					lookups.staffOrder,
					(value) => value.id
				),
				homerooms: mergeLookup(
					workspace.homerooms,
					lookups.upsertedHomerooms,
					lookups.removedHomeroomIds,
					lookups.homeroomOrder,
					(value) => value.id
				),
				rooms: mergeLookup(
					workspace.rooms,
					lookups.upsertedRooms,
					lookups.removedRoomIds,
					lookups.roomOrder,
					(value) => value.id
				),
				bellPeriods: mergeLookup(
					workspace.bellPeriods,
					lookups.upsertedBellPeriods,
					lookups.removedBellPeriodIds,
					lookups.bellPeriodOrder,
					(value) => value.id
				),
				offeringSubjectGroups: mergeLookup(
					workspace.offeringSubjectGroups,
					lookups.upsertedOfferingSubjectGroups,
					lookups.removedSubjectGroupOfferingIds,
					lookups.subjectGroupOfferingOrder,
					(value) => value.learningOfferingId
				)
			}
		: {};
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
		...lookupWorkspace,
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
