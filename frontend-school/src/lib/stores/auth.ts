import { writable } from 'svelte/store';
import { setPermissions, clearPermissions } from './permissions';
import type { SessionState } from '../auth/session-activity.js';

export interface User {
	id: string;
	username?: string;
	firstName: string;
	lastName: string;
	displayName: string;
	role: string;
	user_type?: string; // 'staff' | 'student'
	status: string;
	primaryRoleName?: string; // ชื่อบทบาทหลักจากฐานข้อมูล
	profileImageFileId?: string;
}

export interface AuthState {
	user: User | null;
	session: SessionState | null;
	isAuthenticated: boolean;
	isLoading: boolean;
	isUnavailable: boolean;
}

function createAuthStore() {
	let generation = 0;
	let sessionEpoch = 0;
	let identity = '';
	let currentSession: SessionState | null = null;
	const { subscribe, set, update } = writable<AuthState>({
		user: null,
		session: null,
		isAuthenticated: false,
		isLoading: true,
		isUnavailable: false
	});

	return {
		subscribe,
		get generation() {
			return generation;
		},
		get sessionEpoch() {
			return sessionEpoch;
		},
		setUser: (user: User, permissions: string[], session?: SessionState) => {
			const nextSession =
				session ??
				(identity.startsWith(`${user.id}:${user.user_type ?? ''}:`) ? currentSession : null);
			const nextIdentity = `${user.id}:${user.user_type ?? ''}:${nextSession?.sessionId ?? ''}`;
			if (identity !== nextIdentity) sessionEpoch++;
			identity = nextIdentity;
			if (
				!currentSession ||
				currentSession.sessionId !== nextSession?.sessionId ||
				(nextSession && Date.parse(nextSession.serverNow) >= Date.parse(currentSession.serverNow))
			)
				currentSession = nextSession;
			generation++;
			set({
				user,
				session: currentSession,
				isAuthenticated: true,
				isLoading: false,
				isUnavailable: false
			});

			setPermissions(permissions);
		},
		updateSession: (session: SessionState, expectedEpoch: number) => {
			if (
				expectedEpoch !== sessionEpoch ||
				currentSession?.sessionId !== session.sessionId ||
				Date.parse(session.serverNow) < Date.parse(currentSession.serverNow)
			)
				return;
			currentSession = session;
			update((state) => ({ ...state, session }));
		},
		clearUser: () => {
			sessionEpoch++;
			identity = '';
			currentSession = null;
			generation++;
			set({
				user: null,
				session: null,
				isAuthenticated: false,
				isLoading: false,
				isUnavailable: false
			});

			// Auto-clear permissions when user logs out
			clearPermissions();
		},
		setLoading: (loading: boolean) => {
			update((state) => ({ ...state, isLoading: loading }));
		},
		setUnavailable: () => {
			update((state) => ({
				...state,
				isLoading: false,
				isUnavailable: true
			}));
		}
	};
}

export const authStore = createAuthStore();
