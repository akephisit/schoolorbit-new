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
			this.initialize();
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

	// Initialize from backend (Cookie check only - no localStorage)
	private async initialize() {
		this.state.isLoading = true;

		try {
			// Verify session with backend via HttpOnly cookie
			const response = await apiClient.getCurrentUser();
			if (response.success && response.data) {
				const { user } = response.data;
				this.state.user = user;
				this.state.isAuthenticated = true;
			} else {
				// Cookie invalid or expired
				this.state.user = null;
				this.state.isAuthenticated = false;
			}
		} catch (error) {
			console.error('Failed to initialize auth:', error);
			this.state.user = null;
			this.state.isAuthenticated = false;
		} finally {
			this.state.isLoading = false;
		}
	}

	// No longer saving to localStorage - security first!
	// User data exists only in memory and HttpOnly cookie

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
	async logout() {
		try {
			await apiClient.logout();
		} catch (error) {
			console.error('Logout API call failed:', error);
		}

		this.state.user = null;
		this.state.isAuthenticated = false;
		this.state.error = null;

		if (browser) {
			goto('/login');
		}
	}

	// Clear error
	clearError() {
		this.state.error = null;
	}
}

export const authStore = new AuthStore();
