// API Client base
import { browser } from '$app/environment';
import { resolve } from '$app/paths';
import { PUBLIC_BACKEND_URL } from '$env/static/public';

export const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';
export const BACKEND_WS_URL = BACKEND_URL.replace(/^http/, 'ws');

export interface ApiResponse<T> {
	success: boolean;
	data?: T;
	error?: string;
	message?: string;
}

export function requireApiData<T>(response: ApiResponse<T>, fallbackError: string): T {
	if (!response.success || response.data === undefined) {
		throw new Error(response.error || fallbackError);
	}

	return response.data;
}

class APIClient {
	private baseURL: string;

	constructor(baseURL: string) {
		this.baseURL = baseURL;
	}

	private async parseResponse(response: Response): Promise<unknown> {
		const contentType = response.headers.get('content-type') ?? '';
		const text = await response.text();

		if (!text) {
			return {};
		}

		if (contentType.includes('application/json')) {
			try {
				return JSON.parse(text);
			} catch {
				return { error: 'รูปแบบข้อมูลจากเซิร์ฟเวอร์ไม่ถูกต้อง' };
			}
		}

		return { error: text };
	}

	private errorMessage(data: unknown): string {
		if (data && typeof data === 'object') {
			const payload = data as { error?: unknown; message?: unknown };
			if (typeof payload.error === 'string' && payload.error) return payload.error;
			if (typeof payload.message === 'string' && payload.message) return payload.message;
		}

		return 'เกิดข้อผิดพลาด';
	}

	private handleUnauthorized() {
		if (!browser) return;

		const loginPath = resolve('/login');
		const currentPath = `${window.location.pathname}${window.location.search}${window.location.hash}`;

		if (currentPath.startsWith(loginPath)) return;

		sessionStorage.setItem('redirectAfterLogin', currentPath);
		window.location.assign(loginPath);
	}

	private async request<T>(endpoint: string, options: RequestInit = {}): Promise<ApiResponse<T>> {
		const url = `${this.baseURL}${endpoint}`;

		const headers = new Headers(options.headers);
		if (options.body !== undefined && !headers.has('Content-Type')) {
			headers.set('Content-Type', 'application/json');
		}

		const response = await fetch(url, {
			...options,
			credentials: 'include',
			headers
		});

		const data = await this.parseResponse(response);

		if (!response.ok) {
			if (response.status === 401) {
				this.handleUnauthorized();
			}

			return {
				success: false,
				error: this.errorMessage(data)
			};
		}

		return data as ApiResponse<T>;
	}

	async get<T>(endpoint: string): Promise<ApiResponse<T>> {
		return this.request<T>(endpoint, { method: 'GET' });
	}

	async post<T>(endpoint: string, body?: unknown): Promise<ApiResponse<T>> {
		return this.request<T>(endpoint, {
			method: 'POST',
			body: body ? JSON.stringify(body) : undefined
		});
	}

	async put<T>(endpoint: string, body?: unknown): Promise<ApiResponse<T>> {
		return this.request<T>(endpoint, {
			method: 'PUT',
			body: body ? JSON.stringify(body) : undefined
		});
	}

	async patch<T>(endpoint: string, body?: unknown): Promise<ApiResponse<T>> {
		return this.request<T>(endpoint, {
			method: 'PATCH',
			body: body ? JSON.stringify(body) : undefined
		});
	}

	async delete<T>(endpoint: string): Promise<ApiResponse<T>> {
		return this.request<T>(endpoint, { method: 'DELETE' });
	}

	async deleteWithBody<T>(endpoint: string, body: unknown): Promise<ApiResponse<T>> {
		return this.request<T>(endpoint, {
			method: 'DELETE',
			body: JSON.stringify(body)
		});
	}

	async postMultipart<T>(endpoint: string, body: FormData): Promise<ApiResponse<T>> {
		const url = `${this.baseURL}${endpoint}`;
		// Do NOT set Content-Type — browser sets it with the multipart boundary automatically
		const response = await fetch(url, {
			method: 'POST',
			credentials: 'include',
			body
		});
		const data = await this.parseResponse(response);
		if (!response.ok) {
			if (response.status === 401) {
				this.handleUnauthorized();
			}

			return { success: false, error: this.errorMessage(data) };
		}
		return data as ApiResponse<T>;
	}
}

export const apiClient = new APIClient(BACKEND_URL);
