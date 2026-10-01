import { writable } from 'svelte/store';
import { setPermissions, clearPermissions } from './permissions';

export interface User {
	id: string;
	username?: string;
	firstName: string;
	lastName: string;
	role: string;
	user_type?: string; // 'staff' | 'student'
	status: string;
	primaryRoleName?: string; // ชื่อบทบาทหลักจากฐานข้อมูล
	profileImageFileId?: string;
}

export interface AuthState {
	user: User | null;
	isAuthenticated: boolean;
	isLoading: boolean;
	isUnavailable: boolean;
}

function createAuthStore() {
	let generation = 0;
	let sessionEpoch = 0;
	let identity = '';
	const { subscribe, set, update } = writable<AuthState>({
		user: null,
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
		setUser: (user: User, permissions: string[]) => {
			const nextIdentity = `${user.id}:${user.user_type ?? ''}`;
			if (identity !== nextIdentity) sessionEpoch++;
			identity = nextIdentity;
			generation++;
			set({
				user,
				isAuthenticated: true,
				isLoading: false,
				isUnavailable: false
			});

			setPermissions(permissions);
		},
		clearUser: () => {
			sessionEpoch++;
			identity = '';
			generation++;
			set({
				user: null,
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
