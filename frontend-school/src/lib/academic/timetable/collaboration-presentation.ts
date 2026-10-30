/** A grouped roster suitable for recognition and choosing between a person's open tabs. */
export interface TimetableCollaborator {
	userId: string;
	name: string;
	color: string;
	profileImageFileId: string | null;
	tabs: { connectionId: string; contextLabel: string; activityLabel: string; canJump: boolean }[];
}

/** A cursor resolved to an element in this browser, never to the sender's screen pixels. */
export interface TimetableRemotePointer {
	connectionId: string;
	name: string;
	color: string;
	subjectKey: string;
	contextKey: string;
	subject: HTMLElement;
	position: RelativePoint;
	ghost: { code: string | null; title: string } | null;
}

export function collaboratorInitials(name: string): string {
	const parts = name.trim().split(/\s+/u).filter(Boolean);
	return parts.length > 1 ? `${parts[0][0]}${parts.at(-1)![0]}` : (parts[0] ?? '?').slice(0, 2);
}
import type { RelativePoint } from './collaboration-geometry.js';
import type {
	DraftAvailability,
	TimetableBlockPlacementSource,
	TimetableBlockWorkspaceRoom
} from '../../api/timetable.js';
import { sameDraftSource, type OwnedDraftLease } from './collaboration-session.ts';

/** Private room availability is useful only for the exact currently owned edit. */
export function timetableEditRoomOptions(
	rooms: TimetableBlockWorkspaceRoom[],
	source: TimetableBlockPlacementSource | null,
	lease: OwnedDraftLease | null,
	availability: DraftAvailability | null,
	originalRoomId: string | null = null
): { ready: boolean; rooms: TimetableBlockWorkspaceRoom[]; retainedRoomId: string | null } {
	if (
		!source ||
		source.kind !== 'existing_block' ||
		!lease?.valid ||
		lease.purpose !== 'edit' ||
		!sameDraftSource(source, lease.source) ||
		!availability?.roomOptions ||
		!sameDraftSource(source, availability.source) ||
		availability.roomOptions.reservationId !== lease.reservationId
	)
		return { ready: false, rooms: [], retainedRoomId: null };
	const availableIds = new Set(availability.roomOptions.availableRoomIds);
	return {
		ready: true,
		retainedRoomId:
			originalRoomId &&
			availableIds.has(originalRoomId) &&
			rooms.some((room) => room.id === originalRoomId)
				? originalRoomId
				: null,
		rooms: rooms.filter(
			(room) => room.status.toUpperCase() === 'ACTIVE' && availableIds.has(room.id)
		)
	};
}
