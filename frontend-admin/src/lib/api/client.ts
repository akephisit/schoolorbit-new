// API Client for backend-admin
import { PUBLIC_API_URL } from '$env/static/public';
import { authStore } from '$lib/stores/auth.svelte';

const API_BASE_URL = PUBLIC_API_URL;

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
					authStore.logout();
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

	// School Management
	async createSchool(data: CreateSchool): Promise<ApiResponse<School>> {
		return this.request<School>('/api/v1/schools', {
			method: 'POST',
			body: JSON.stringify(data)
		});
	}

	async listSchools(page = 1, limit = 10): Promise<ApiResponse<SchoolListResponse>> {
		return this.request<SchoolListResponse>(`/api/v1/schools?page=${page}&limit=${limit}`, {
			method: 'GET'
		});
	}

	async getSchool(id: string): Promise<ApiResponse<School>> {
		return this.request<School>(`/api/v1/schools/${id}`, {
			method: 'GET'
		});
	}

	async updateSchool(id: string, data: UpdateSchool): Promise<ApiResponse<School>> {
		return this.request<School>(`/api/v1/schools/${id}`, {
			method: 'PUT',
			body: JSON.stringify(data)
		});
	}

	async deleteSchool(id: string): Promise<ApiResponse<void>> {
		return this.request<void>(`/api/v1/schools/${id}`, {
			method: 'DELETE'
		});
	}

	// Deployment methods
	async deploySchool(id: string): Promise<ApiResponse<DeployResponse>> {
		return this.request<DeployResponse>(`/api/v1/schools/${id}/deploy`, {
			method: 'POST'
		});
	}

	async bulkDeploySchools(schoolIds: string[]): Promise<ApiResponse<BulkDeployResult>> {
		return this.request<BulkDeployResult>('/api/v1/schools/deploy/bulk', {
			method: 'POST',
			body: JSON.stringify({ school_ids: schoolIds })
		});
	}

	async getDeploymentHistory(id: string): Promise<ApiResponse<DeploymentHistory[]>> {
		return this.request<DeploymentHistory[]>(`/api/v1/schools/${id}/deployments`);
	}
}

export const apiClient = new ApiClient();

// Types
export interface CreateSchool {
	name: string;
	subdomain: string;
	adminUsername: string;
	adminPassword: string;
}

export interface UpdateSchool {
	name?: string;
	status?: string;
	config?: Record<string, any>;
}

export interface School {
	id: string;
	name: string;
	subdomain: string;
	dbName: string;
	dbConnectionString: string | null;
	status: string;
	config: Record<string, any>;
	createdAt: string;
	updatedAt: string;
	// Deployment tracking
	deploymentStatus?: 'provisioning' | 'active' | 'deployment_failed' | 'failed';
	subdomainUrl?: string;
	deploymentErrorMessage?: string | null;
}

export interface DeployResponse {
	success: boolean;
	message: string;
	deploymentUrl?: string;
	githubActionsUrl?: string;
}

export interface DeployResult {
	schoolId: string;
	schoolName: string;
	success: boolean;
	message: string;
	deploymentUrl?: string;
}

export interface BulkDeployResult {
	total: number;
	successful: DeployResult[];
	failed: DeployResult[];
}

export interface DeploymentHistory {
	id: string;
	schoolId: string;
	status: string;
	message?: string;
	githubRunId?: string;
	githubRunUrl?: string;
	createdAt: string;
	completedAt?: string;
}

export interface SchoolListResponse {
	schools: School[];
	total: number;
	page: number;
	limit: number;
	totalPages: number;
}
