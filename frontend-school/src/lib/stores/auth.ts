import { writable } from 'svelte/store';

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
		},
		clearUser: () => {
			set({
				user: null,
				isAuthenticated: false,
				isLoading: false
			});
		},
		setLoading: (loading: boolean) => {
			update((state) => ({ ...state, isLoading: loading }));
		}
	};
}

export const authStore = createAuthStore();
