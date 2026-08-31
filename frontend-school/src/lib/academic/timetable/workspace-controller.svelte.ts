import type {
	TimetablePlacementCandidate,
	TimetablePlacementMutationKind,
	TimetablePlacementPreview,
	TimetablePlacementSource,
	TimetableWorkspace
} from '../../api/timetable';
import {
	createTimetableBoardState,
	remainingDemandForGroup,
	rowsForTimetableView,
	type TimetableBoardView
} from './board-state';

export interface TimetableDragSource {
	source: TimetablePlacementSource;
	candidate: TimetablePlacementCandidate;
}

class TimetableWorkspaceController {
	workspace!: TimetableWorkspace;
	view = $state<TimetableBoardView>('homeroom');
	selectedOwnerId!: string | null;
	dragSource = $state.raw<TimetableDragSource | null>(null);
	preview = $state.raw<TimetablePlacementPreview | null>(null);
	pendingMutation = $state<TimetablePlacementMutationKind | null>(null);
	isRefreshing = $state(false);

	board = $derived.by(() => createTimetableBoardState(this.workspace));
	rows = $derived.by(() => rowsForTimetableView(this.board, this.view));
	selectedRow = $derived(this.rows.find((row) => row.id === this.selectedOwnerId) ?? null);
	canEdit = $derived(this.board.canEdit && this.pendingMutation === null);

	constructor(workspace: TimetableWorkspace) {
		this.workspace = $state.raw(workspace);
		this.selectedOwnerId = $state(this.initialOwnerId('homeroom'));
	}

	setWorkspace = (workspace: TimetableWorkspace) => {
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

	startPlacement = (source: TimetablePlacementSource, candidate: TimetablePlacementCandidate) => {
		this.dragSource = { source, candidate };
		this.preview = null;
	};

	setPreview = (preview: TimetablePlacementPreview | null) => {
		this.preview = preview;
	};

	beginMutation = (mutation: TimetablePlacementMutationKind) => {
		this.pendingMutation = mutation;
	};

	finishMutation = () => {
		this.pendingMutation = null;
		this.clearPlacement();
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
		return rowsForTimetableView(this.board, view)[0]?.id ?? null;
	}
}

export function createTimetableWorkspaceController(workspace: TimetableWorkspace) {
	return new TimetableWorkspaceController(workspace);
}

export type { TimetableWorkspaceController };
