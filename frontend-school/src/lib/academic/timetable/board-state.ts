import type {
	TimetableBlock,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementSource,
	TimetableBlockPlacementState,
	TimetableBlockPlacementPreview,
	TimetableBlockWorkspace,
	TimetableOrdinaryDemand
} from '../../api/timetable';
import { orderTimetableHomerooms } from './demand-ordering.ts';

export type TimetableBoardView = 'homeroom' | 'learning_group' | 'teacher';
export type TimetablePageView = TimetableBoardView | 'wholeSchool';
export type LocalTimetableConflict = TimetableBlockPlacementPreview['conflicts'][number];
type ResourceKind = 'learning_group' | 'homeroom' | 'teacher' | 'room';
type PlacementResources = Record<ResourceKind, string[]>;

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

export function patchTimetableWorkspaceBlocks(
	workspace: TimetableBlockWorkspace,
	changedBlocks: readonly TimetableBlock[]
): TimetableBlockWorkspace {
	const changedById = new Map(changedBlocks.map((block) => [block.id, block]));
	const existingIds = new Set(workspace.blocks.map((block) => block.id));
	const blocks = [
		...workspace.blocks.map((block) => changedById.get(block.id) ?? block),
		...changedBlocks.filter((block) => !existingIds.has(block.id))
	];
	return setTimetableWorkspaceBlocks(workspace, blocks);
}

export function setTimetableWorkspaceBlocks(
	workspace: TimetableBlockWorkspace,
	blocks: readonly TimetableBlock[]
): TimetableBlockWorkspace {
	const activeBlocks = blocks.filter((block) => block.isActive);

	return {
		...workspace,
		blocks: [...blocks],
		ordinaryDemands: workspace.ordinaryDemands.map((demand) => {
			const scheduledPeriods = activeBlocks.filter((block) =>
				block.groups.some((group) => group.learningGroupId === demand.learningGroupId)
			).length;
			return {
				...demand,
				scheduledPeriods,
				remainingPeriods: Math.max(demand.requiredPeriods - scheduledPeriods, 0)
			};
		}),
		synchronizedDemands: workspace.synchronizedDemands.map((demand) => ({
			...demand,
			scheduledPeriods: activeBlocks.filter(
				(block) =>
					block.learningOfferingId === demand.learningOfferingId &&
					block.schedulingMode === 'synchronized'
			).length
		})),
		summary: {
			...workspace.summary,
			blockCount: activeBlocks.length
		}
	};
}

export function rowsForTimetableView(
	state: TimetableBoardState,
	view: TimetableBoardView
): TimetableBoardRow[] {
	if (view === 'homeroom') {
		return orderTimetableHomerooms(state.workspace.homerooms).map((homeroom) => ({
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
	const sourceBlock =
		request.source.kind === 'existing_block'
			? state.blocksById.get(request.source.blockId)
			: undefined;
	if (
		sourceBlock?.dayOfWeek === request.dayOfWeek &&
		sourceBlock.bellSchedulePeriodId === request.bellSchedulePeriodId
	) {
		return { state: 'source', targetBlockId: null, conflicts: [] };
	}
	const slotBlocks = state.blocks.filter(
		(block) =>
			block.isActive !== false &&
			block.id !== sourceBlock?.id &&
			block.dayOfWeek === request.dayOfWeek &&
			block.bellSchedulePeriodId === request.bellSchedulePeriodId
	);
	const targetBlocks = slotBlocks.filter((block) =>
		blockBelongsToRow(block, request.view, request.rowId)
	);
	const target = targetBlocks[0];
	const resources = candidateResources(state, request.candidate, sourceBlock);
	if (sourceBlock && target && targetBlocks.length === 1) {
		const others = state.blocks.filter(
			(block) => block.isActive !== false && block.id !== sourceBlock.id && block.id !== target.id
		);
		const conflicts = [
			...localConflicts(
				resources,
				others.filter(
					(block) =>
						block.dayOfWeek === request.dayOfWeek &&
						block.bellSchedulePeriodId === request.bellSchedulePeriodId
				)
			),
			...localConflicts(
				blockResources(target),
				others.filter(
					(block) =>
						block.dayOfWeek === sourceBlock.dayOfWeek &&
						block.bellSchedulePeriodId === sourceBlock.bellSchedulePeriodId
				),
				true
			)
		];
		return { state: conflicts.length ? 'blocked' : 'swap', targetBlockId: target.id, conflicts };
	}
	const conflicts = localConflicts(resources, slotBlocks);
	return {
		state: conflicts.length || target ? 'blocked' : 'move',
		targetBlockId: target?.id ?? null,
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

function candidateResources(
	state: TimetableBoardState,
	candidate: TimetableBlockPlacementCandidate,
	sourceBlock?: TimetableBlock
): PlacementResources {
	if (sourceBlock) return blockResources(sourceBlock);
	return {
		learning_group: candidate.learningGroupId ? [candidate.learningGroupId] : [],
		homeroom: candidate.learningGroupId
			? (state.groupsById.get(candidate.learningGroupId)?.homeroomIds ??
				candidate.homeroomIds ??
				[])
			: (candidate.homeroomIds ?? []),
		teacher: [...(candidate.instructorIds ?? []), ...(candidate.teacherIds ?? [])],
		room: candidate.roomId ? [candidate.roomId] : []
	};
}

function blockResources(block: TimetableBlock): PlacementResources {
	return {
		learning_group: block.groups.map((group) => group.learningGroupId),
		homeroom: blockHomeroomIds(block),
		teacher: blockTeacherIds(block),
		room: [...block.groups, ...block.homerooms].flatMap((target) =>
			target.roomId ? [target.roomId] : []
		)
	};
}

function localConflicts(
	resources: PlacementResources,
	blocks: readonly TimetableBlock[],
	returning = false
): LocalTimetableConflict[] {
	const conflicts: LocalTimetableConflict[] = [];
	const labels: Record<ResourceKind, string> = {
		learning_group: 'กลุ่มเรียนมีคาบอยู่แล้ว',
		homeroom: 'ห้องเรียนมีคาบอยู่แล้ว',
		teacher: 'ครูมีคาบอยู่แล้ว',
		room: 'ห้องสอนถูกใช้อยู่แล้ว'
	};
	for (const block of blocks) {
		const occupied = blockResources(block);
		for (const kind of ['learning_group', 'homeroom', 'teacher', 'room'] as const) {
			for (const id of new Set(resources[kind])) {
				if (!occupied[kind].includes(id)) continue;
				conflicts.push({
					conflictType: kind,
					code: `TIMETABLE_${returning ? 'SWAP_RETURN_' : ''}${kind === 'learning_group' ? 'GROUP' : kind.toUpperCase()}_CONFLICT`,
					message: `${returning ? 'คาบที่สลับกลับ: ' : ''}${labels[kind]}`,
					existingBlockId: block.id,
					targetKind: kind === 'learning_group' ? 'group' : kind === 'room' ? null : kind,
					targetId: id
				});
			}
		}
	}
	return conflicts;
}

export function blockTeacherIds(block: TimetableBlock): string[] {
	return [...blockInstructorIds(block), ...blockTargetTeacherIds(block)].filter(
		(teacherId, index, values) => values.indexOf(teacherId) === index
	);
}

export function blockInstructorIds(block: TimetableBlock): string[] {
	return block.groups
		.flatMap((group) => group.instructors.map((teacher) => teacher.teacherId))
		.filter((teacherId, index, values) => values.indexOf(teacherId) === index);
}

export function blockTargetTeacherIds(block: TimetableBlock): string[] {
	return block.teachers
		.map((teacher) => teacher.teacherId)
		.filter((teacherId, index, values) => values.indexOf(teacherId) === index);
}

export function blockHomeroomIds(block: TimetableBlock): string[] {
	return [
		...block.groups.flatMap((group) => group.homeroomIds),
		...block.homerooms.map((target) => target.homeroomId)
	].filter((homeroomId, index, values) => values.indexOf(homeroomId) === index);
}
