import type {
	TimetableEntry,
	TimetablePlacementCandidate,
	TimetablePlacementSource,
	TimetablePlacementState,
	TimetableUnscheduledDemand,
	TimetableWorkspace
} from '../../api/timetable';

export type TimetableBoardView = 'homeroom' | 'learning_group' | 'teacher';
export type TimetablePageView = TimetableBoardView | 'wholeSchool';
export type LocalTimetableConflict = 'learning_group' | 'homeroom' | 'instructor' | 'room';

export interface TimetableBoardRow {
	id: string;
	code: string;
	label: string;
	kind: TimetableBoardView;
}

export interface TimetableCellAddress {
	view: TimetableBoardView;
	rowId: string;
	dayOfWeek: string;
	bellSchedulePeriodId: string;
}

export interface LocalPlacementRequest extends TimetableCellAddress {
	source: TimetablePlacementSource;
	candidate: TimetablePlacementCandidate;
}

export interface LocalPlacementPreview {
	state: TimetablePlacementState;
	targetEntryId: string | null;
	conflicts: LocalTimetableConflict[];
}

export interface TimetableBoardState {
	workspace: TimetableWorkspace;
	entries: TimetableEntry[];
	entriesById: ReadonlyMap<string, TimetableEntry>;
	groupsById: ReadonlyMap<string, TimetableWorkspace['learningGroups'][number]>;
	homeroomsById: ReadonlyMap<string, TimetableWorkspace['homerooms'][number]>;
	demandsByGroupId: ReadonlyMap<string, TimetableUnscheduledDemand>;
	canEdit: boolean;
}

export function createTimetableBoardState(workspace: TimetableWorkspace): TimetableBoardState {
	const entries = [...workspace.entries];
	return {
		workspace,
		entries,
		entriesById: new Map(entries.map((entry) => [entry.id, entry])),
		groupsById: new Map(workspace.learningGroups.map((group) => [group.id, group])),
		homeroomsById: new Map(workspace.homerooms.map((homeroom) => [homeroom.id, homeroom])),
		demandsByGroupId: new Map(
			workspace.unscheduledDemands.map((demand) => [demand.learningGroupId, demand])
		),
		canEdit: workspace.version.status === 'draft'
	};
}

export function replaceTimetableEntries(
	state: TimetableBoardState,
	entries: readonly TimetableEntry[]
): TimetableBoardState {
	return createTimetableBoardState({
		...state.workspace,
		entries: [...entries]
	});
}

export function rowsForTimetableView(
	state: TimetableBoardState,
	view: TimetableBoardView
): TimetableBoardRow[] {
	if (view === 'homeroom') {
		return state.workspace.homerooms.map((homeroom) => ({
			id: homeroom.id,
			code: homeroom.code,
			label: homeroom.name,
			kind: view
		}));
	}
	if (view === 'teacher') {
		return state.workspace.staff.map((teacher) => ({
			id: teacher.id,
			code: 'ครู',
			label: teacher.displayName,
			kind: view
		}));
	}
	return state.workspace.learningGroups.map((group) => ({
		id: group.id,
		code: group.code,
		label: group.name,
		kind: view
	}));
}

export function entriesForTimetableCell(
	state: TimetableBoardState,
	address: TimetableCellAddress
): TimetableEntry[] {
	return state.entries.filter(
		(entry) =>
			entry.dayOfWeek === address.dayOfWeek &&
			entry.bellSchedulePeriodId === address.bellSchedulePeriodId &&
			entryBelongsToRow(state, entry, address.view, address.rowId)
	);
}

export function remainingDemandForGroup(state: TimetableBoardState, groupId: string): number {
	const demand = state.demandsByGroupId.get(groupId);
	if (!demand) return 0;
	const scheduled = state.entries.reduce(
		(count, entry) => count + (entry.learningGroupId === groupId ? 1 : 0),
		0
	);
	return Math.max(demand.requiredPeriods - scheduled, 0);
}

export function teacherPeriodCount(state: TimetableBoardState, teacherId: string): number {
	return state.entries.reduce(
		(count, entry) =>
			count + (entry.instructors.some((instructor) => instructor.userId === teacherId) ? 1 : 0),
		0
	);
}

export function localPlacementPreview(
	state: TimetableBoardState,
	request: LocalPlacementRequest
): LocalPlacementPreview {
	const sourceEntryId =
		request.source.kind === 'existing_entry' ? request.source.entryId : undefined;
	const slotEntries = state.entries.filter(
		(entry) =>
			entry.id !== sourceEntryId &&
			entry.dayOfWeek === request.dayOfWeek &&
			entry.bellSchedulePeriodId === request.bellSchedulePeriodId
	);
	const targetEntries = slotEntries.filter((entry) =>
		entryBelongsToRow(state, entry, request.view, request.rowId)
	);
	const conflicts = localConflicts(state, request.candidate, slotEntries);

	if (targetEntries.length > 0) {
		return {
			state: request.source.kind === 'existing_entry' ? 'swap' : 'blocked',
			targetEntryId: targetEntries[0]?.id ?? null,
			conflicts
		};
	}
	return {
		state: conflicts.length > 0 ? 'blocked' : 'move',
		targetEntryId: null,
		conflicts
	};
}

function entryBelongsToRow(
	state: TimetableBoardState,
	entry: TimetableEntry,
	view: TimetableBoardView,
	rowId: string
): boolean {
	if (view === 'learning_group') return entry.learningGroupId === rowId;
	if (view === 'teacher') {
		return entry.instructors.some((instructor) => instructor.userId === rowId);
	}
	if (entry.learningGroupId) {
		return state.groupsById.get(entry.learningGroupId)?.homeroomIds.includes(rowId) ?? false;
	}
	return entry.homeroomId === rowId;
}

function localConflicts(
	state: TimetableBoardState,
	candidate: TimetablePlacementCandidate,
	entries: readonly TimetableEntry[]
): LocalTimetableConflict[] {
	const candidateHomerooms = candidate.learningGroupId
		? (state.groupsById.get(candidate.learningGroupId)?.homeroomIds ?? [])
		: candidate.homeroomId
			? [candidate.homeroomId]
			: [];
	const candidateInstructors = new Set(candidate.instructorIds ?? []);
	const conflicts = new Set<LocalTimetableConflict>();

	for (const entry of entries) {
		if (candidate.learningGroupId && candidate.learningGroupId === entry.learningGroupId) {
			conflicts.add('learning_group');
		}
		const entryHomerooms = entry.learningGroupId
			? (state.groupsById.get(entry.learningGroupId)?.homeroomIds ?? [])
			: entry.homeroomId
				? [entry.homeroomId]
				: [];
		if (candidateHomerooms.some((homeroomId) => entryHomerooms.includes(homeroomId))) {
			conflicts.add('homeroom');
		}
		if (candidate.roomId && candidate.roomId === entry.roomId) conflicts.add('room');
		if (entry.instructors.some((instructor) => candidateInstructors.has(instructor.userId))) {
			conflicts.add('instructor');
		}
	}
	return ['learning_group', 'homeroom', 'instructor', 'room'].filter((conflict) =>
		conflicts.has(conflict as LocalTimetableConflict)
	) as LocalTimetableConflict[];
}
