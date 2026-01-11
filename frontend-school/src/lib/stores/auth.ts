import { writable } from 'svelte/store';
import { loadUserPermissions, clearPermissions } from './permissions';

export interface User {
	id: string;
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
	permissions?: string[]; // Optional - will be loaded from permission store
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

			// Auto-load permissions when user is set
			if (user?.id) {
				loadUserPermissions(user.id).catch((error) => {
					console.error('Failed to auto-load permissions:', error);
				});
			}
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
