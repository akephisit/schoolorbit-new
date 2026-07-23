// API Client base
import { browser } from '$app/environment';
import { resolve } from '$app/paths';
import { env } from '$env/dynamic/public';
import { PUBLIC_BACKEND_URL } from '$env/static/public';

export const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';
export const BACKEND_WS_URL = BACKEND_URL.replace(/^http/, 'ws');
const SCHOOL_SUBDOMAIN_HEADER = 'X-School-Subdomain';

export interface ApiResponse<T> {
	success: boolean;
	data?: T;
	error?: string;
	message?: string;
}

export interface ApiRequestOptions {
	signal?: AbortSignal;
}

const INVALID_API_RESPONSE_ERROR = 'รูปแบบข้อมูลจากเซิร์ฟเวอร์ไม่ถูกต้อง';

function isRecord(value: unknown): value is Record<string, unknown> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function normalizeApiResponse<T>(data: unknown): ApiResponse<T> {
	if (!isRecord(data)) {
		return { success: false, error: INVALID_API_RESPONSE_ERROR };
	}

	const payload = data;
	const message = typeof payload.message === 'string' ? payload.message : undefined;

	if (typeof payload.success !== 'boolean') {
		return { success: false, error: INVALID_API_RESPONSE_ERROR, message };
	}

	if (!payload.success) {
		const error =
			typeof payload.error === 'string' && payload.error
				? payload.error
				: (message ?? 'เกิดข้อผิดพลาด');
		return { success: false, error, message };
	}

	if (!('data' in payload)) {
		return { success: false, error: INVALID_API_RESPONSE_ERROR, message };
	}

	return { success: true, data: payload.data as T, message };
}

function normalizeSchoolSubdomain(value: string | undefined): string | null {
	const subdomain = value?.trim().toLowerCase();
	if (!subdomain || subdomain === 'www') return null;
	if (!/^[a-z0-9-]+$/.test(subdomain)) return null;
	return subdomain;
}

function getRequestSubdomain(): string | null {
	return normalizeSchoolSubdomain(env.PUBLIC_SCHOOL_SUBDOMAIN);
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

	private applyTenantHeader(headers: Headers) {
		if (headers.has(SCHOOL_SUBDOMAIN_HEADER)) return;

		const subdomain = getRequestSubdomain();
		if (subdomain) {
			headers.set(SCHOOL_SUBDOMAIN_HEADER, subdomain);
		}
	}

	private async request<T>(endpoint: string, options: RequestInit = {}): Promise<ApiResponse<T>> {
		const url = `${this.baseURL}${endpoint}`;

		const headers = new Headers(options.headers);
		if (options.body !== undefined && !headers.has('Content-Type')) {
			headers.set('Content-Type', 'application/json');
		}
		this.applyTenantHeader(headers);

		const response = await fetch(url, {
			...options,
			credentials: 'include',
			headers
		});

		const data = await this.parseResponse(response);
		const normalized = normalizeApiResponse<T>(data);

		if (!response.ok) {
			if (response.status === 401) {
				this.handleUnauthorized();
			}

			return {
				success: false,
				error: normalized.error ?? this.errorMessage(data),
				message: normalized.message
			};
		}

		return normalized;
	}

	async get<T>(endpoint: string, options: ApiRequestOptions = {}): Promise<ApiResponse<T>> {
		return this.request<T>(endpoint, { method: 'GET', signal: options.signal });
	}

	async getBlob(endpoint: string, options: ApiRequestOptions = {}): Promise<ApiResponse<Blob>> {
		const url = `${this.baseURL}${endpoint}`;
		const headers = new Headers();
		this.applyTenantHeader(headers);
		const response = await fetch(url, {
			method: 'GET',
			credentials: 'include',
			headers,
			signal: options.signal
		});
		if (response.ok) return { success: true, data: await response.blob() };

		const data = await this.parseResponse(response);
		const normalized = normalizeApiResponse<Blob>(data);
		if (response.status === 401) this.handleUnauthorized();
		return {
			success: false,
			error: normalized.error ?? this.errorMessage(data),
			message: normalized.message
		};
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
		const headers = new Headers();
		this.applyTenantHeader(headers);
		// Do NOT set Content-Type — browser sets it with the multipart boundary automatically
		const response = await fetch(url, {
			method: 'POST',
			credentials: 'include',
			headers,
			body
		});
		const data = await this.parseResponse(response);
		const normalized = normalizeApiResponse<T>(data);
		if (!response.ok) {
			if (response.status === 401) {
				this.handleUnauthorized();
			}

			return {
				success: false,
				error: normalized.error ?? this.errorMessage(data),
				message: normalized.message
			};
		}
		return normalized;
	}
}

export const apiClient = new APIClient(BACKEND_URL);
