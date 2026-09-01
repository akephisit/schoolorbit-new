import type {
	TimetableBlockMutationKind,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementPreview,
	TimetableBlockPlacementSource,
	TimetableBlockWorkspace
} from '../../api/timetable';
import {
	createTimetableBoardState,
	remainingDemandForGroup,
	rowsForTimetableView,
	type TimetableBoardView
} from './board-state';

export interface TimetableDragSource {
	source: TimetableBlockPlacementSource;
	candidate: TimetableBlockPlacementCandidate;
}

class TimetableWorkspaceController {
	workspace!: TimetableBlockWorkspace;
	view = $state<TimetableBoardView>('homeroom');
	selectedOwnerId!: string | null;
	dragSource = $state.raw<TimetableDragSource | null>(null);
	preview = $state.raw<TimetableBlockPlacementPreview | null>(null);
	pendingMutation = $state<TimetableBlockMutationKind | null>(null);
	isRefreshing = $state(false);

	board = $derived.by(() => createTimetableBoardState(this.workspace));
	rows = $derived.by(() => rowsForTimetableView(this.board, this.view));
	selectedRow = $derived(this.rows.find((row) => row.id === this.selectedOwnerId) ?? null);
	canEdit = $derived(this.board.canEdit && this.pendingMutation === null);

	constructor(workspace: TimetableBlockWorkspace) {
		this.workspace = $state.raw(workspace);
		this.selectedOwnerId = $state(this.initialOwnerId('homeroom'));
	}

	setWorkspace = (workspace: TimetableBlockWorkspace) => {
		this.workspace = workspace;
		if (!this.rows.some((row) => row.id === this.selectedOwnerId)) {
			this.selectedOwnerId = this.initialOwnerId(this.view);
		}
	};

	setView = (view: TimetableBoardView) => {
		this.view = view;
		this.selectedOwnerId = this.initialOwnerId(view);
		this.clearPlacement();
	};

	selectOwner = (ownerId: string) => {
		if (this.rows.some((row) => row.id === ownerId)) this.selectedOwnerId = ownerId;
	};

	startPlacement = (
		source: TimetableBlockPlacementSource,
		candidate: TimetableBlockPlacementCandidate
	) => {
		this.dragSource = { source, candidate };
		this.preview = null;
	};

	setPreview = (preview: TimetableBlockPlacementPreview | null) => {
		this.preview = preview;
	};

	beginMutation = (mutation: TimetableBlockMutationKind) => {
		this.pendingMutation = mutation;
	};

	finishMutation = () => {
		this.pendingMutation = null;
		this.clearPlacement();
	};

	failMutation = () => {
		this.pendingMutation = null;
	};

	setRefreshing = (refreshing: boolean) => {
		this.isRefreshing = refreshing;
	};

	clearPlacement = () => {
		this.dragSource = null;
		this.preview = null;
	};

	remainingDemand = (groupId: string) => remainingDemandForGroup(this.board, groupId);

	private initialOwnerId(view: TimetableBoardView): string | null {
		if (view === 'teacher') {
			const scheduledTeacherId = this.workspace.blocks
				.flatMap((block) => [
					...block.groups.flatMap((group) => group.instructors.map((teacher) => teacher.teacherId)),
					...block.teachers.map((teacher) => teacher.teacherId)
				])
				.find((teacherId) => this.workspace.staff.some((teacher) => teacher.id === teacherId));
			if (scheduledTeacherId) return scheduledTeacherId;
		}
		return rowsForTimetableView(this.board, view)[0]?.id ?? null;
	}
}

export function createTimetableWorkspaceController(workspace: TimetableBlockWorkspace) {
	return new TimetableWorkspaceController(workspace);
}

export type { TimetableWorkspaceController };
