import { writable } from 'svelte/store';
import { setPermissions, clearPermissions } from './permissions';

export interface User {
	id: string;
	username?: string;
	nationalId?: string;
	email?: string;
	firstName: string;
	lastName: string;
	role: string;
	user_type?: string; // 'staff' | 'student'
	phone?: string;
	status: string;
	createdAt: string;
	primaryRoleName?: string; // ชื่อบทบาทหลักจากฐานข้อมูล
	profileImageUrl?: string;
	permissions?: string[]; // Permissions from /api/auth/me
}

export interface AuthState {
	user: User | null;
	isAuthenticated: boolean;
	isLoading: boolean;
}

function createAuthStore() {
	const { subscribe, set, update } = writable<AuthState>({
		user: null,
		isAuthenticated: false,
		isLoading: true
	});

	return {
		subscribe,
		setUser: (user: User) => {
			set({
				user,
				isAuthenticated: true,
				isLoading: false
			});

			// Sync permissions from user object (from /api/auth/me)
			// This avoids 403 error from /api/users/{id}/permissions
			setPermissions(user?.permissions);
		},
		clearUser: () => {
			set({
				user: null,
				isAuthenticated: false,
				isLoading: false
			});

			// Auto-clear permissions when user logs out
			clearPermissions();
		},
		setLoading: (loading: boolean) => {
			update((state) => ({ ...state, isLoading: loading }));
		}
	};
}

export const authStore = createAuthStore();
