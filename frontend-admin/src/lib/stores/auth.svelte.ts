// Auth Store using Svelte 5 runes

import { apiClient, type LoginRequest, type LoginResponse } from '$lib/api/client';
import { goto } from '$app/navigation';
import { browser } from '$app/environment';

interface User {
	id: string;
	nationalId: string;
	name: string;
	role: string;
}

interface AuthState {
	user: User | null;
	isAuthenticated: boolean;
	isLoading: boolean;
	error: string | null;
}

class AuthStore {
	private state = $state<AuthState>({
		user: null,
		isAuthenticated: false,
		isLoading: false,
		error: null
	});

	constructor() {
		if (browser) {
			this.initializeFromStorage();
		}
	}

	// Getters
	get user() {
		return this.state.user;
	}

	get isAuthenticated() {
		return this.state.isAuthenticated;
	}

	get isLoading() {
		return this.state.isLoading;
	}

	get error() {
		return this.state.error;
	}

	// Initialize from localStorage (User data only)
	private initializeFromStorage() {
		try {
			const userStr = localStorage.getItem('auth_user');

			if (userStr) {
				const user = JSON.parse(userStr);
				this.state.user = user;
				this.state.isAuthenticated = true;
			}
		} catch (error) {
			console.error('Failed to initialize auth from storage:', error);
			this.clearStorage();
		}
	}

	// Save to localStorage (User data only)
	private saveToStorage(user: User) {
		try {
			localStorage.setItem('auth_user', JSON.stringify(user));
		} catch (error) {
			console.error('Failed to save auth to storage:', error);
		}
	}

	// Clear localStorage
	private clearStorage() {
		try {
			localStorage.removeItem('auth_user');
		} catch (error) {
			console.error('Failed to clear auth storage:', error);
		}
	}

	// Login
	async login(credentials: LoginRequest) {
		this.state.isLoading = true;
		this.state.error = null;

		try {
			const response = await apiClient.login(credentials);

			if (response.success && response.data) {
				const { user } = response.data;

				this.state.user = user;
				this.state.isAuthenticated = true;

				if (browser) {
					this.saveToStorage(user);
				}

				// Redirect to dashboard
				await goto('/dashboard');

				return { success: true };
			} else {
				this.state.error = response.error || 'Login failed';
				return { success: false, error: this.state.error };
			}
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : 'An error occurred';
			this.state.error = errorMessage;
			return { success: false, error: errorMessage };
		} finally {
			this.state.isLoading = false;
		}
	}

	// Logout
	logout() {
		this.state.user = null;
		this.state.isAuthenticated = false;
		this.state.error = null;

		if (browser) {
			this.clearStorage();
			goto('/login');
		}
	}

	// Clear error
	clearError() {
		this.state.error = null;
	}
}

export const authStore = new AuthStore();
