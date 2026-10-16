import { PUBLIC_API_URL } from '$app/env/public';

export interface ApiResponse<T> {
	success: boolean;
	data?: T;
	error?: string;
	message?: string;
}
export interface LoginRequest {
	email: string;
	password: string;
}
export interface Account {
	id: string;
	email: string;
	name: string;
	role: 'operator' | 'customer';
}
export interface LoginResponse {
	user: Account;
	csrfToken: string;
}
export interface CustomerSchool {
	id: string;
	name: string;
	subdomain: string;
	cadence: string | null;
	endsAt: string | null;
	expired: boolean;
}
export interface ServiceRequestInput {
	id: string;
	schoolId: string | null;
	kind: 'open_school' | 'renew_school';
	schoolName: string;
	subdomain: string;
	cadence: 'monthly' | 'yearly';
}
export interface ServiceRequest extends ServiceRequestInput {
	accountId: string;
	email: string;
	status: 'pending' | 'approved' | 'rejected';
	decisionReason: string | null;
	createdAt: string;
}

class ApiClient {
	private csrfToken = '';
	csrfHeaders(): Record<string, string> {
		return this.csrfToken ? { 'X-CSRF-Token': this.csrfToken } : {};
	}
	clearSession() {
		this.csrfToken = '';
	}
	async raw<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
		const response = await fetch(`${PUBLIC_API_URL}${endpoint}`, {
			...options,
			credentials: 'include',
			cache: 'no-store',
			headers: { 'Content-Type': 'application/json', ...this.csrfHeaders(), ...options.headers }
		});
		if (!response.ok) {
			const body: { error?: string } = await response.json().catch(() => ({}));
			throw new Error(body.error || `ไม่สามารถดำเนินการได้ (${response.status})`);
		}
		return response.json();
	}
	async request<T>(endpoint: string, options: RequestInit = {}): Promise<ApiResponse<T>> {
		try {
			return await this.raw<ApiResponse<T>>(endpoint, options);
		} catch (error) {
			return {
				success: false,
				error: error instanceof Error ? error.message : 'ไม่สามารถเชื่อมต่อบริการได้'
			};
		}
	}
	async login(credentials: LoginRequest) {
		const result = await this.request<LoginResponse>('/api/v1/auth/login', {
			method: 'POST',
			body: JSON.stringify(credentials)
		});
		if (result.success && result.data) this.csrfToken = result.data.csrfToken;
		return result;
	}
	async getCurrentUser() {
		const result = await this.request<LoginResponse>('/api/v1/auth/me');
		if (result.success && result.data) this.csrfToken = result.data.csrfToken;
		else this.clearSession();
		return result;
	}
	async logout() {
		const result = await this.request<void>('/api/v1/auth/logout', { method: 'POST' });
		if (result.success) this.clearSession();
		return result;
	}
	register(input: { email: string; password: string; name: string }) {
		return this.request<void>('/api/v1/auth/register', {
			method: 'POST',
			body: JSON.stringify(input)
		});
	}
	emailAction(action: 'resend-verification' | 'forgot-password', email: string) {
		return this.request<void>(`/api/v1/auth/${action}`, {
			method: 'POST',
			body: JSON.stringify({ email })
		});
	}
	verify(token: string) {
		return this.request<void>('/api/v1/auth/verify-email', {
			method: 'POST',
			body: JSON.stringify({ token })
		});
	}
	resetPassword(token: string, password: string) {
		return this.request<void>('/api/v1/auth/reset-password', {
			method: 'POST',
			body: JSON.stringify({ token, password })
		});
	}
	customerSchools() {
		return this.request<CustomerSchool[]>('/api/v1/portal/schools');
	}
	serviceRequests() {
		return this.request<ServiceRequest[]>('/api/v1/portal/requests');
	}
	createServiceRequest(input: ServiceRequestInput) {
		return this.request<void>('/api/v1/portal/requests', {
			method: 'POST',
			body: JSON.stringify(input)
		});
	}
	decideRequest(id: string, input: { approve: boolean; schoolId: string | null; reason: string }) {
		return this.request<void>(`/api/v1/operator/requests/${id}/decision`, {
			method: 'POST',
			body: JSON.stringify(input)
		});
	}
	assignSchool(id: string, email: string) {
		return this.request<void>(`/api/v1/operator/schools/${id}/members`, {
			method: 'POST',
			body: JSON.stringify({ email })
		});
	}
	async healthCheck() {
		try {
			return (await fetch(`${PUBLIC_API_URL}/health`)).ok;
		} catch {
			return false;
		}
	}
	createSchool(data: CreateSchool): Promise<ApiResponse<School>> {
		return this.request('/api/v1/schools', { method: 'POST', body: JSON.stringify(data) });
	}
	listSchools(page = 1, limit = 10): Promise<ApiResponse<SchoolListResponse>> {
		return this.request(`/api/v1/schools?page=${page}&limit=${limit}`);
	}
	getSchool(id: string): Promise<ApiResponse<School>> {
		return this.request(`/api/v1/schools/${id}`);
	}
	updateSchool(id: string, data: UpdateSchool): Promise<ApiResponse<School>> {
		return this.request(`/api/v1/schools/${id}`, { method: 'PUT', body: JSON.stringify(data) });
	}
	deploySchool(id: string): Promise<ApiResponse<DeployResponse>> {
		return this.request(`/api/v1/schools/${id}/deploy`, { method: 'POST' });
	}
	bulkDeploySchools(schoolIds: string[]): Promise<ApiResponse<BulkDeployResult>> {
		return this.request('/api/v1/schools/deploy/bulk', {
			method: 'POST',
			body: JSON.stringify({ school_ids: schoolIds })
		});
	}
	getDeploymentHistory(id: string): Promise<ApiResponse<DeploymentHistory[]>> {
		return this.request(`/api/v1/schools/${id}/deployments`);
	}
}
export const apiClient = new ApiClient();

// Types
export interface CreateSchool {
	name: string;
	subdomain: string;
	adminUsername?: string;
	adminPassword: string;
	adminTitle: string;
	adminFirstName: string;
	adminLastName: string;
}

export interface UpdateSchool {
	name?: string;
	status?: string;
	config?: SchoolConfig;
}

export interface SchoolConfig {
	deployment_url?: string;
	error?: string;
	[key: string]: unknown;
}

export interface School {
	id: string;
	name: string;
	subdomain: string;
	dbName: string;
	status: string;
	config: SchoolConfig;
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
