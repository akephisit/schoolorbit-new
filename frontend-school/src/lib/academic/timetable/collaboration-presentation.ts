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
