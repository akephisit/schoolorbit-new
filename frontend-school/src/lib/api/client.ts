// API Client base
import { browser } from '$app/environment';
import { resolve } from '$app/paths';
import { env } from '$env/dynamic/public';
import { PUBLIC_BACKEND_URL } from '$env/static/public';
import {
	captureSessionSecurityHeaders,
	clearSessionSecurity,
	retryAfterSeconds as parseRetryAfterSeconds,
	withSessionSecurityHeaders
} from '$lib/api/session-security';
import { appendApiQuery, type ApiQuery } from '$lib/api/query';
import { normalizeSchoolSubdomain } from '$lib/api/school-subdomain';
import { authStore } from '$lib/stores/auth';

export const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';
export const BACKEND_WS_URL = BACKEND_URL.replace(/^http/, 'ws');
const SCHOOL_SUBDOMAIN_HEADER = 'X-School-Subdomain';

export interface ApiResponse<T, E = never> {
	success: boolean;
	data?: T;
	errorData?: E;
	error?: string;
	message?: string;
	status: number;
	retryAfterSeconds?: number;
}

export interface ApiRequestOptions {
	signal?: AbortSignal;
	query?: ApiQuery;
}

type ApiTransport = 'session' | 'public';

export class ApiClientError<E = never> extends Error {
	constructor(
		message: string,
		readonly status: number,
		readonly retryAfterSeconds?: number,
		readonly data?: E
	) {
		super(message);
		this.name = 'ApiClientError';
	}
}

const INVALID_API_RESPONSE_ERROR = 'รูปแบบข้อมูลจากเซิร์ฟเวอร์ไม่ถูกต้อง';

function isRecord(value: unknown): value is Record<string, unknown> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function normalizeApiResponse<T, E = never>(
	data: unknown,
	status: number,
	retryAfterSeconds?: number
): ApiResponse<T, E> {
	if (!isRecord(data)) {
		return { success: false, error: INVALID_API_RESPONSE_ERROR, status, retryAfterSeconds };
	}

	const payload = data;
	const message = typeof payload.message === 'string' ? payload.message : undefined;

	if (typeof payload.success !== 'boolean') {
		return {
			success: false,
			error: INVALID_API_RESPONSE_ERROR,
			message,
			status,
			retryAfterSeconds
		};
	}

	if (!payload.success) {
		const error =
			typeof payload.error === 'string' && payload.error
				? payload.error
				: (message ?? 'เกิดข้อผิดพลาด');
		if ('data' in payload) {
			return {
				success: false,
				error,
				errorData: payload.data as E,
				message,
				status,
				retryAfterSeconds
			};
		}
		return { success: false, error, message, status, retryAfterSeconds };
	}

	if (!('data' in payload)) {
		return {
			success: false,
			error: INVALID_API_RESPONSE_ERROR,
			message,
			status,
			retryAfterSeconds
		};
	}

	return {
		success: true,
		data: payload.data as T,
		message,
		status,
		retryAfterSeconds
	};
}

export function getSchoolSubdomainHint(): string | null {
	return normalizeSchoolSubdomain(env.PUBLIC_SCHOOL_SUBDOMAIN);
}

export function requireApiData<T, E = never>(
	response: ApiResponse<T, E>,
	fallbackError: string
): T {
	if (!response.success || response.data === undefined) {
		throw new ApiClientError<E>(
			response.error || fallbackError,
			response.status,
			response.retryAfterSeconds,
			response.errorData
		);
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

		if (!text) return {};

		if (contentType.includes('application/json')) {
			try {
				return JSON.parse(text);
			} catch {
				return { error: INVALID_API_RESPONSE_ERROR };
			}
		}

		return { error: text };
	}

	private errorMessage(data: unknown): string {
		if (isRecord(data)) {
			if (typeof data.error === 'string' && data.error) return data.error;
			if (typeof data.message === 'string' && data.message) return data.message;
		}

		return 'เกิดข้อผิดพลาด';
	}

	private handleUnauthorized(): void {
		clearSessionSecurity();
		authStore.clearUser();
		if (!browser) return;

		const loginPath = resolve('/login');
		const currentUrl = new URL(window.location.href);
		if (currentUrl.pathname === loginPath || currentUrl.pathname === `${loginPath}/`) return;

		const currentPath = `${currentUrl.pathname}${currentUrl.search}${currentUrl.hash}`;
		const redirectTarget = new URL(currentPath, window.location.origin);
		if (redirectTarget.origin === window.location.origin && currentPath.startsWith('/')) {
			sessionStorage.setItem('redirectAfterLogin', currentPath);
		}
		window.location.assign(loginPath);
	}

	private async fetchBackend(
		endpoint: string,
		options: RequestInit = {},
		transport: ApiTransport = 'session'
	): Promise<Response> {
		const method = (options.method ?? 'GET').toUpperCase();
		const callerHeaders = new Headers(options.headers);
		callerHeaders.delete(SCHOOL_SUBDOMAIN_HEADER);
		const usesSession = transport === 'session';
		const headers = usesSession ? withSessionSecurityHeaders(method, callerHeaders) : callerHeaders;
		const subdomain = getSchoolSubdomainHint();
		if (subdomain) headers.set(SCHOOL_SUBDOMAIN_HEADER, subdomain);

		const requestOptions: RequestInit = {
			...options,
			method,
			credentials: usesSession ? 'include' : 'omit',
			headers
		};
		if (!usesSession) {
			requestOptions.referrerPolicy = 'no-referrer';
			requestOptions.cache = 'no-store';
		}
		const response = await fetch(`${this.baseURL}${endpoint}`, requestOptions);
		if (usesSession) {
			captureSessionSecurityHeaders(response.headers);
			if (response.status === 401) this.handleUnauthorized();
		}
		return response;
	}

	private responseMetadata(response: Response): {
		status: number;
		retryAfterSeconds?: number;
	} {
		return {
			status: response.status,
			retryAfterSeconds: parseRetryAfterSeconds(response.headers)
		};
	}

	private async request<T, E = never>(
		endpoint: string,
		options: RequestInit = {},
		transport: ApiTransport = 'session'
	): Promise<ApiResponse<T, E>> {
		const headers = new Headers(options.headers);
		if (options.body !== undefined && !headers.has('Content-Type')) {
			headers.set('Content-Type', 'application/json');
		}

		const response = await this.fetchBackend(endpoint, { ...options, headers }, transport);
		const data = await this.parseResponse(response);
		const metadata = this.responseMetadata(response);
		const normalized = normalizeApiResponse<T, E>(
			data,
			metadata.status,
			metadata.retryAfterSeconds
		);

		if (!response.ok) {
			return {
				...normalized,
				success: false,
				error: normalized.error ?? this.errorMessage(data)
			};
		}

		return normalized;
	}

	private async blobResponse(response: Response): Promise<ApiResponse<Blob>> {
		const metadata = this.responseMetadata(response);
		if (response.ok) {
			return { success: true, data: await response.blob(), ...metadata };
		}

		const data = await this.parseResponse(response);
		const normalized = normalizeApiResponse<Blob>(
			data,
			metadata.status,
			metadata.retryAfterSeconds
		);
		return {
			...normalized,
			success: false,
			error: normalized.error ?? this.errorMessage(data)
		};
	}

	async get<T, E = never>(
		endpoint: string,
		options: ApiRequestOptions = {}
	): Promise<ApiResponse<T, E>> {
		return this.request<T, E>(appendApiQuery(endpoint, options.query), {
			method: 'GET',
			signal: options.signal
		});
	}

	async getBlob(endpoint: string, options: ApiRequestOptions = {}): Promise<ApiResponse<Blob>> {
		const response = await this.fetchBackend(appendApiQuery(endpoint, options.query), {
			method: 'GET',
			signal: options.signal
		});
		return this.blobResponse(response);
	}

	async getExternalBlob(url: string, options: ApiRequestOptions = {}): Promise<ApiResponse<Blob>> {
		const response = await fetch(url, {
			method: 'GET',
			mode: 'cors',
			credentials: 'omit',
			referrerPolicy: 'no-referrer',
			signal: options.signal
		});
		const metadata = this.responseMetadata(response);
		if (response.ok) return { success: true, data: await response.blob(), ...metadata };

		return {
			success: false,
			error: `ดาวน์โหลดไฟล์ไม่สำเร็จ (${response.status})`,
			...metadata
		};
	}

	async post<T, E = never>(
		endpoint: string,
		body?: unknown,
		options: ApiRequestOptions = {}
	): Promise<ApiResponse<T, E>> {
		return this.request<T, E>(endpoint, {
			method: 'POST',
			body: body === undefined ? undefined : JSON.stringify(body),
			signal: options.signal
		});
	}

	async postPublic<T, E = never>(
		endpoint: string,
		body: unknown,
		options: ApiRequestOptions = {}
	): Promise<ApiResponse<T, E>> {
		return this.request<T, E>(
			endpoint,
			{
				method: 'POST',
				body: JSON.stringify(body),
				signal: options.signal
			},
			'public'
		);
	}

	async postBlob(endpoint: string, options: ApiRequestOptions = {}): Promise<ApiResponse<Blob>> {
		const response = await this.fetchBackend(endpoint, {
			method: 'POST',
			signal: options.signal
		});
		return this.blobResponse(response);
	}

	async postBlobWithBody(
		endpoint: string,
		body: unknown,
		options: ApiRequestOptions = {}
	): Promise<ApiResponse<Blob>> {
		const response = await this.fetchBackend(endpoint, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify(body),
			signal: options.signal
		});
		return this.blobResponse(response);
	}

	async put<T, E = never>(endpoint: string, body?: unknown): Promise<ApiResponse<T, E>> {
		return this.request<T, E>(endpoint, {
			method: 'PUT',
			body: body === undefined ? undefined : JSON.stringify(body)
		});
	}

	async patch<T, E = never>(endpoint: string, body?: unknown): Promise<ApiResponse<T, E>> {
		return this.request<T, E>(endpoint, {
			method: 'PATCH',
			body: body === undefined ? undefined : JSON.stringify(body)
		});
	}

	async delete<T, E = never>(endpoint: string): Promise<ApiResponse<T, E>> {
		return this.request<T, E>(endpoint, { method: 'DELETE' });
	}

	async deleteWithBody<T, E = never>(endpoint: string, body: unknown): Promise<ApiResponse<T, E>> {
		return this.request<T, E>(endpoint, {
			method: 'DELETE',
			body: JSON.stringify(body)
		});
	}

	async postMultipart<T, E = never>(endpoint: string, body: FormData): Promise<ApiResponse<T, E>> {
		const response = await this.fetchBackend(endpoint, {
			method: 'POST',
			body
		});
		const data = await this.parseResponse(response);
		const metadata = this.responseMetadata(response);
		const normalized = normalizeApiResponse<T, E>(
			data,
			metadata.status,
			metadata.retryAfterSeconds
		);
		if (!response.ok) {
			return {
				...normalized,
				success: false,
				error: normalized.error ?? this.errorMessage(data)
			};
		}
		return normalized;
	}
}

export const apiClient = new APIClient(BACKEND_URL);
