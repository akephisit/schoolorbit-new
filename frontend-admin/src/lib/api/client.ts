// API Client for backend-admin

const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:8080';

export interface ApiResponse<T> {
	success: boolean;
	data?: T;
	error?: string;
}

export interface LoginRequest {
	nationalId: string;
	password: string;
}

export interface LoginResponse {
	// Token is handled via HttpOnly cookie
	user: {
		id: string;
		nationalId: string;
		name: string;
		role: string;
	};
}

class ApiClient {
	private baseUrl: string;

	constructor(baseUrl: string = API_BASE_URL) {
		this.baseUrl = baseUrl;
	}

	private async request<T>(
		endpoint: string,
		options: RequestInit = {}
	): Promise<ApiResponse<T>> {
		const url = `${this.baseUrl}${endpoint}`;

		try {
			const response = await fetch(url, {
				...options,
				credentials: 'include', // Important for HttpOnly cookies
				headers: {
					'Content-Type': 'application/json',
					...options.headers
				}
			});

			const data = await response.json();

			if (!response.ok) {
				// Auto logout on 401 Unauthorized (except for /me endpoint during initialization)
				// /me is expected to return 401 when not logged in
				if (response.status === 401 && typeof window !== 'undefined' && !url.includes('/auth/me')) {
					console.warn('Session expired (401), logging out...');
					// Import authStore dynamically to avoid circular dependency
					import('$lib/stores/auth.svelte').then(({ authStore }) => {
						authStore.logout();
					});
				}

				return {
					success: false,
					error: data.error || `HTTP error! status: ${response.status}`
				};
			}

			return data as ApiResponse<T>;
		} catch (error) {
			console.error('API request failed:', error);
			return {
				success: false,
				error: error instanceof Error ? error.message : 'Network error'
			};
		}
	}

	async login(credentials: LoginRequest): Promise<ApiResponse<LoginResponse>> {
		return this.request<LoginResponse>('/api/v1/auth/login', {
			method: 'POST',
			body: JSON.stringify(credentials)
		});
	}

	async logout(): Promise<ApiResponse<void>> {
		return this.request<void>('/api/v1/auth/logout', {
			method: 'POST'
		});
	}

	async getCurrentUser(): Promise<ApiResponse<LoginResponse>> {
		return this.request<LoginResponse>('/api/v1/auth/me', {
			method: 'GET'
		});
	}

	async healthCheck(): Promise<boolean> {
		try {
			const response = await fetch(`${this.baseUrl}/health`);
			return response.ok;
		} catch {
			return false;
		}
	}
}

export const apiClient = new ApiClient();
