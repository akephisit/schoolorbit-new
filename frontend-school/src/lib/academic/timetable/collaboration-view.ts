import type {
	DraftAnchor,
	DraftContext,
	DraftParticipant,
	DraftReservation,
	TimetableBlockWorkspace
} from '../../api/timetable.js';
import type {
	TimetableCollaborator,
	TimetableRemotePointer
} from './collaboration-presentation.js';
import {
	findTimetableBlockElement,
	findTimetableCellElement,
	findTimetableDemandElement
} from './collaboration-dom.ts';
import { sourceGhost } from './collaboration-source.ts';

const dayNames: Record<string, string> = {
	MON: 'จันทร์',
	TUE: 'อังคาร',
	WED: 'พุธ',
	THU: 'พฤหัสบดี',
	FRI: 'ศุกร์',
	SAT: 'เสาร์',
	SUN: 'อาทิตย์'
};

export function collaborationContextLabel(
	workspace: TimetableBlockWorkspace,
	context: DraftContext | null | undefined
): string | null {
	if (!context) return null;
	switch (context.viewMode) {
		case 'homeroom': {
			const row = workspace.homerooms.find((item) => item.id === context.viewId);
			return row ? `ห้อง ${row.name}` : null;
		}
		case 'teacher': {
			const row = workspace.staff.find((item) => item.id === context.viewId);
			return row ? row.displayName : null;
		}
		case 'learning_group': {
			const row = workspace.learningGroups.find((item) => item.id === context.viewId);
			return row ? `กลุ่ม ${row.name}` : null;
		}
		case 'wholeSchool': {
			const row = context.viewId
				? workspace.homerooms.find((item) => item.id === context.viewId)
				: null;
			if (context.viewId && !row) return null;
			return `ภาพรวมทั้งโรงเรียน${context.dayOfWeek ? ` · วัน${dayNames[context.dayOfWeek] ?? ''}` : ''}${row ? ` · ${row.name}` : ''}`;
		}
	}
}

export function timetableCollaborators(
	workspace: TimetableBlockWorkspace,
	participants: DraftParticipant[],
	ownConnectionId: string | null,
	reservations: DraftReservation[]
): TimetableCollaborator[] {
	return participants
		.map((participant) => ({
			userId: participant.userId,
			name: participant.name,
			color: participant.color,
			profileImageFileId: participant.profileImageFileId ?? null,
			tabs: participant.connections
				.filter((connection) => connection.connectionId !== ownConnectionId)
				.map((connection) => {
					const reservation = reservations.find(
						(item) => item.reservationId === connection.activeReservationId
					);
					return {
						connectionId: connection.connectionId,
						contextLabel:
							collaborationContextLabel(workspace, connection.context) ?? 'กำลังเปิดตาราง',
						canJump: collaborationContextLabel(workspace, connection.context) !== null,
						activityLabel:
							reservation?.purpose === 'edit'
								? 'กำลังแก้ไขรายละเอียดคาบ'
								: reservation
									? 'กำลังจัดคาบ'
									: 'กำลังดูตาราง'
					};
				})
		}))
		.filter((participant) => participant.tabs.length > 0);
}

export function collaborationContextsMatch(local: DraftContext, remote: DraftContext): boolean {
	if (local.viewMode === 'wholeSchool')
		return Boolean(
			remote.viewId &&
			local.dayOfWeek &&
			(remote.viewMode === 'homeroom' ||
				(remote.viewMode === 'wholeSchool' && remote.dayOfWeek === local.dayOfWeek))
		);
	if (!local.viewId || remote.viewId !== local.viewId) return false;
	return (
		remote.viewMode === local.viewMode ||
		(local.viewMode === 'homeroom' &&
			remote.viewMode === 'wholeSchool' &&
			Boolean(remote.dayOfWeek))
	);
}

export function resolveCollaborationAnchor(
	root: HTMLElement,
	local: DraftContext,
	remote: DraftContext,
	anchor: DraftAnchor
): HTMLElement | null {
	if (!collaborationContextsMatch(local, remote)) return null;
	if (anchor.kind === 'block') {
		const element = findTimetableBlockElement(
			root,
			anchor.blockId,
			local.viewMode === 'wholeSchool' && ['homeroom', 'wholeSchool'].includes(remote.viewMode)
				? remote.viewId
				: null
		);
		if (
			remote.viewMode === 'wholeSchool' &&
			element?.closest<HTMLElement>('[data-timetable-anchor="cell"]')?.dataset.timetableDay !==
				remote.dayOfWeek
		)
			return null;
		return element;
	}
	if (anchor.kind === 'tray')
		return local.viewMode === 'wholeSchool' || remote.viewMode === 'wholeSchool'
			? null
			: findTimetableDemandElement(root, anchor.source);
	if (remote.viewMode === 'wholeSchool' && anchor.dayOfWeek !== remote.dayOfWeek) return null;
	if (local.viewMode === 'wholeSchool') {
		if (
			!['homeroom', 'wholeSchool'].includes(remote.viewMode) ||
			!remote.viewId ||
			local.dayOfWeek !== anchor.dayOfWeek
		)
			return null;
		return findTimetableCellElement(
			root,
			anchor.dayOfWeek,
			anchor.bellSchedulePeriodId,
			remote.viewId
		);
	}
	return findTimetableCellElement(
		root,
		anchor.dayOfWeek,
		anchor.bellSchedulePeriodId,
		local.viewId
	);
}

export function timetableRemotePointers(
	root: HTMLElement,
	workspace: TimetableBlockWorkspace,
	local: DraftContext,
	participants: DraftParticipant[],
	ownConnectionId: string | null,
	reservations: DraftReservation[]
): TimetableRemotePointer[] {
	return participants.flatMap((participant) =>
		participant.connections.flatMap((connection) => {
			if (
				connection.connectionId === ownConnectionId ||
				!connection.anchor ||
				!connection.context ||
				!collaborationContextLabel(workspace, connection.context)
			)
				return [];
			const subject = resolveCollaborationAnchor(
				root,
				local,
				connection.context,
				connection.anchor
			);
			if (!subject) return [];
			const reservation = reservations.find(
				(item) => item.reservationId === connection.activeReservationId
			);
			const anchor = connection.anchor;
			const subjectKey =
				anchor.kind === 'block'
					? `block:${anchor.blockId}`
					: anchor.kind === 'cell'
						? `cell:${connection.context.viewMode}:${connection.context.viewId}:${anchor.dayOfWeek}:${anchor.bellSchedulePeriodId}`
						: `tray:${JSON.stringify(anchor.source)}`;
			return [
				{
					connectionId: connection.connectionId,
					name: participant.name,
					color: participant.color,
					subjectKey,
					contextKey: `${connection.context.viewMode}:${connection.context.viewId ?? ''}:${connection.context.viewMode === 'wholeSchool' ? (connection.context.dayOfWeek ?? '') : ''}`,
					subject,
					position: { u: anchor.u, v: anchor.v },
					ghost: reservation?.purpose === 'drag' ? sourceGhost(workspace, reservation.source) : null
				}
			];
		})
	);
}
