import type {
	TimetableBlock,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementSource,
	TimetableBlockPlacementState,
	TimetableBlockWorkspace,
	TimetableOrdinaryDemand
} from '../../api/timetable';

export type TimetableBoardView = 'homeroom' | 'learning_group' | 'teacher';
export type TimetablePageView = TimetableBoardView | 'wholeSchool';
export type LocalTimetableConflict = 'learning_group' | 'homeroom' | 'teacher' | 'room';

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
	source: TimetableBlockPlacementSource;
	candidate: TimetableBlockPlacementCandidate;
}

export interface LocalPlacementPreview {
	state: TimetableBlockPlacementState;
	targetBlockId: string | null;
	conflicts: LocalTimetableConflict[];
}

export interface TimetableBoardState {
	workspace: TimetableBlockWorkspace;
	blocks: TimetableBlock[];
	blocksById: ReadonlyMap<string, TimetableBlock>;
	groupsById: ReadonlyMap<string, TimetableBlockWorkspace['learningGroups'][number]>;
	homeroomsById: ReadonlyMap<string, TimetableBlockWorkspace['homerooms'][number]>;
	demandsByGroupId: ReadonlyMap<string, TimetableOrdinaryDemand>;
	canEdit: boolean;
}

export function createTimetableBoardState(workspace: TimetableBlockWorkspace): TimetableBoardState {
	const blocks = [...workspace.blocks];
	return {
		workspace,
		blocks,
		blocksById: new Map(blocks.map((block) => [block.id, block])),
		groupsById: new Map(workspace.learningGroups.map((group) => [group.id, group])),
		homeroomsById: new Map(workspace.homerooms.map((homeroom) => [homeroom.id, homeroom])),
		demandsByGroupId: new Map(
			workspace.ordinaryDemands.map((demand) => [demand.learningGroupId, demand])
		),
		canEdit: workspace.version.status === 'draft'
	};
}

export function replaceTimetableBlocks(
	state: TimetableBoardState,
	blocks: readonly TimetableBlock[]
): TimetableBoardState {
	return createTimetableBoardState({
		...state.workspace,
		blocks: [...blocks]
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

export function blocksForTimetableCell(
	state: TimetableBoardState,
	address: TimetableCellAddress
): TimetableBlock[] {
	return state.blocks.filter(
		(block) =>
			block.dayOfWeek === address.dayOfWeek &&
			block.bellSchedulePeriodId === address.bellSchedulePeriodId &&
			blockBelongsToRow(block, address.view, address.rowId)
	);
}

export function remainingDemandForGroup(state: TimetableBoardState, groupId: string): number {
	const demand = state.demandsByGroupId.get(groupId);
	if (!demand) return 0;
	const scheduled = state.blocks.reduce(
		(count, block) =>
			count + (block.groups.some((group) => group.learningGroupId === groupId) ? 1 : 0),
		0
	);
	return Math.max(demand.requiredPeriods - scheduled, 0);
}

export function teacherPeriodCount(state: TimetableBoardState, teacherId: string): number {
	return state.blocks.reduce(
		(count, block) => count + (blockTeacherIds(block).includes(teacherId) ? 1 : 0),
		0
	);
}

export function localPlacementPreview(
	state: TimetableBoardState,
	request: LocalPlacementRequest
): LocalPlacementPreview {
	const sourceBlockId =
		request.source.kind === 'existing_block' ? request.source.blockId : undefined;
	const slotBlocks = state.blocks.filter(
		(block) =>
			block.id !== sourceBlockId &&
			block.dayOfWeek === request.dayOfWeek &&
			block.bellSchedulePeriodId === request.bellSchedulePeriodId
	);
	const targetBlocks = slotBlocks.filter((block) =>
		blockBelongsToRow(block, request.view, request.rowId)
	);
	const conflicts = localConflicts(state, request.candidate, slotBlocks);

	if (targetBlocks.length > 0) {
		return {
			state: request.source.kind === 'existing_block' ? 'swap' : 'blocked',
			targetBlockId: targetBlocks[0]?.id ?? null,
			conflicts
		};
	}
	return {
		state: conflicts.length > 0 ? 'blocked' : 'move',
		targetBlockId: null,
		conflicts
	};
}

export function blockBelongsToRow(
	block: TimetableBlock,
	view: TimetableBoardView,
	rowId: string
): boolean {
	if (view === 'learning_group') {
		return block.groups.some((group) => group.learningGroupId === rowId);
	}
	if (view === 'teacher') return blockTeacherIds(block).includes(rowId);
	return (
		block.homerooms.some((target) => target.homeroomId === rowId) ||
		block.groups.some((group) => group.homeroomIds.includes(rowId))
	);
}

function localConflicts(
	state: TimetableBoardState,
	candidate: TimetableBlockPlacementCandidate,
	blocks: readonly TimetableBlock[]
): LocalTimetableConflict[] {
	const candidateHomerooms = candidate.learningGroupId
		? (state.groupsById.get(candidate.learningGroupId)?.homeroomIds ?? [])
		: (candidate.homeroomIds ?? []);
	const candidateTeachers = new Set([
		...(candidate.instructorIds ?? []),
		...(candidate.teacherIds ?? [])
	]);
	const conflicts = new Set<LocalTimetableConflict>();

	for (const block of blocks) {
		if (
			candidate.learningGroupId &&
			block.groups.some((group) => group.learningGroupId === candidate.learningGroupId)
		) {
			conflicts.add('learning_group');
		}
		const blockHomerooms = blockHomeroomIds(block);
		if (candidateHomerooms.some((homeroomId) => blockHomerooms.includes(homeroomId))) {
			conflicts.add('homeroom');
		}
		if (
			candidate.roomId &&
			[...block.groups, ...block.homerooms].some((target) => target.roomId === candidate.roomId)
		) {
			conflicts.add('room');
		}
		if (blockTeacherIds(block).some((teacherId) => candidateTeachers.has(teacherId))) {
			conflicts.add('teacher');
		}
	}
	return ['learning_group', 'homeroom', 'teacher', 'room'].filter((conflict) =>
		conflicts.has(conflict as LocalTimetableConflict)
	) as LocalTimetableConflict[];
}

export function blockTeacherIds(block: TimetableBlock): string[] {
	return [
		...block.groups.flatMap((group) => group.instructors.map((teacher) => teacher.teacherId)),
		...block.teachers.map((teacher) => teacher.teacherId)
	].filter((teacherId, index, values) => values.indexOf(teacherId) === index);
}

export function blockHomeroomIds(block: TimetableBlock): string[] {
	return [
		...block.groups.flatMap((group) => group.homeroomIds),
		...block.homerooms.map((target) => target.homeroomId)
	].filter((homeroomId, index, values) => values.indexOf(homeroomId) === index);
}
