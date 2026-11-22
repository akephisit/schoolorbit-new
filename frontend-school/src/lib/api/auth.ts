import {
	ApiClientError,
	apiClient,
	requireApiData,
	type ApiRequestOptions
} from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';
import { clearSessionSecurity } from '#lib/api/session-security.js';
import { authRefreshDecision, type AuthRefreshResult } from '#lib/auth/auth-refresh-policy.js';
import { announceAuthenticatedSession } from '#lib/auth/session-awareness.js';
import { authStore, type User } from '#lib/stores/auth.js';
import { notificationStore } from '#lib/stores/notification.js';
import { toast } from 'svelte-sonner';
import { get } from 'svelte/store';

type Schemas = components['schemas'];
export type LoginRequest = Schemas['LoginRequest'];
export type ProfileResponse = Schemas['ProfileResponse'];
export type CurrentUserDto = Schemas['CurrentUserResponse'];
export type SessionDto = Schemas['SessionResponse'];
type LoginData = Schemas['LoginData'];
type SessionListData = Schemas['SessionListData'];
type UpdateProfileRequestDto = Schemas['UpdateProfileRequest'];
type ChangePasswordRequestDto = Schemas['ChangePasswordRequest'];
type EmptyData = Schemas['EmptyData'];
export type SessionState = Schemas['SessionState'];

function normalizeCurrentUser(userData: CurrentUserDto): {
	user: User;
	permissions: string[];
	session: SessionState;
} {
	if (
		!userData.session ||
		![
			userData.session.serverNow,
			userData.session.idleExpiresAt,
			userData.session.absoluteExpiresAt,
			userData.session.nextRenewalAt
		].every((value) => Number.isFinite(Date.parse(value))) ||
		!userData.session.sessionId
	) {
		throw new Error('ข้อมูลระยะเวลาการเข้าสู่ระบบไม่ถูกต้อง กรุณาลองอีกครั้ง');
	}
	return {
		user: {
			id: userData.id,
			username: userData.username,
			firstName: userData.firstName,
			lastName: userData.lastName,
			displayName: userData.displayName,
			role: userData.primaryRoleName ?? userData.userType,
			user_type: userData.userType,
			status: userData.status,
			primaryRoleName: userData.primaryRoleName ?? undefined,
			profileImageFileId: userData.profileImageFileId ?? undefined
		},
		permissions: userData.permissions,
		session: userData.session
	};
}

class AuthAPI {
	private refreshVersion = 0;
	private refreshInFlight: { generation: number; promise: Promise<AuthRefreshResult> } | null =
		null;
	private cookieMutation: Promise<unknown> | null = null;

	/** Serialize Set-Cookie responses locally and, where supported, across same-origin tabs. */
	private mutateCookies<T>(operation: () => Promise<T>): Promise<T> {
		const run = () =>
			typeof navigator !== 'undefined' && navigator.locks
				? navigator.locks.request('schoolorbit:auth-cookies', operation)
				: operation();
		const result = this.cookieMutation ? this.cookieMutation.then(run, run) : run();
		// A rejected operation must release the queue; its caller still receives the original error.
		const settled = result.then(
			() => undefined,
			() => undefined
		);
		this.cookieMutation = settled;
		void settled.then(() => {
			if (this.cookieMutation === settled) this.cookieMutation = null;
		});
		return result;
	}

	private currentIntent(intent: number, epoch: number): boolean {
		return intent === authStore.authIntentEpoch && epoch === authStore.sessionEpoch;
	}

	private supersededRefresh(): AuthRefreshResult {
		const current = get(authStore);
		return current.isUnavailable
			? 'unavailable'
			: current.isAuthenticated
				? 'authenticated'
				: 'unauthenticated';
	}

	/** A preceding tab/restore may have selected another finite session while waiting for the lock. */
	private async refreshMutationSecurity(
		intent: number,
		epoch: number,
		allowExpired = false
	): Promise<boolean> {
		if (!this.currentIntent(intent, epoch)) return false;
		const selected = get(authStore);
		const response = await apiClient.get<CurrentUserDto>('/api/auth/me');
		if (!this.currentIntent(intent, epoch)) return false;
		// Logout itself can revoke a presented grant even after its child session expires.
		if (allowExpired && response.status === 401) return true;
		const current = requireApiData(response, 'ไม่สามารถยืนยันการเข้าสู่ระบบได้ กรุณาลองอีกครั้ง');
		if (
			(selected.user &&
				(selected.user.id !== current.id || selected.user.user_type !== current.userType)) ||
			(!allowExpired && selected.session?.sessionId !== current.session.sessionId)
		)
			throw new ApiClientError('การเข้าสู่ระบบเปลี่ยนแปลง กรุณาลองอีกครั้ง', 409);
		return true;
	}
	/**
	 * Login - Direct to backend through the shared client-side API wrapper.
	 */
	async login(data: LoginRequest): Promise<User> {
		const intent = authStore.beginAuthChange();
		const epoch = authStore.sessionEpoch;
		authStore.setLoading(true);

		try {
			return await this.mutateCookies(async () => {
				const changed = () => new ApiClientError('การเข้าสู่ระบบเปลี่ยนแปลง กรุณาลองอีกครั้ง', 409);
				if (!this.currentIntent(intent, epoch)) throw changed();
				const response = await apiClient.post<LoginData>('/api/auth/login', data);
				const userData = requireApiData(response, 'เกิดข้อผิดพลาดในการเข้าสู่ระบบ').user;
				if (!this.currentIntent(intent, epoch)) throw changed();
				const currentUser = normalizeCurrentUser(userData);
				authStore.setUser(currentUser.user, currentUser.permissions, currentUser.session);
				announceAuthenticatedSession(currentUser.session);
				toast.success(response.message || 'เข้าสู่ระบบสำเร็จ');
				return currentUser.user;
			});
		} catch (error: unknown) {
			if (intent !== authStore.authIntentEpoch) throw error;
			if (error instanceof ApiClientError && error.status === 429) {
				const message = error.retryAfterSeconds
					? `มีคำขอเข้าสู่ระบบมากเกินไป กรุณาลองใหม่อีกครั้งใน ${error.retryAfterSeconds} วินาที`
					: 'มีคำขอเข้าสู่ระบบมากเกินไป กรุณาลองใหม่ภายหลัง';
				const sanitizedError = new ApiClientError(message, error.status, error.retryAfterSeconds);
				toast.error(message);
				throw sanitizedError;
			}
			const message =
				error instanceof Error ? error.message : 'ไม่สามารถเชื่อมต่อกับเซิร์ฟเวอร์ได้';
			toast.error(message);
			throw error;
		} finally {
			if (intent === authStore.authIntentEpoch) authStore.setLoading(false);
		}
	}

	/**
	 * Logout - Direct to backend through the shared client-side API wrapper.
	 */
	async logout(): Promise<void> {
		const intent = authStore.beginAuthChange();
		const epoch = authStore.sessionEpoch;
		await this.mutateCookies(async () => {
			if (!(await this.refreshMutationSecurity(intent, epoch, true))) return;
			await notificationStore.prepareLogout();
			if (!this.currentIntent(intent, epoch)) return;
			const response = await apiClient.post<EmptyData>('/api/auth/logout');
			requireApiData(response, 'ออกจากระบบไม่สำเร็จ');
			if (!this.currentIntent(intent, epoch)) return;
			clearSessionSecurity();
			authStore.clearUser();
			toast.success(response.message || 'ออกจากระบบสำเร็จ');
		});
	}

	async listSessions(options: ApiRequestOptions = {}): Promise<SessionDto[]> {
		const response = await apiClient.get<SessionListData>('/api/auth/sessions', options);
		return requireApiData(response, 'ไม่สามารถโหลดรายการอุปกรณ์ได้').sessions;
	}

	async recordActivity(
		expectedSessionId: string,
		options: ApiRequestOptions = {}
	): Promise<SessionState> {
		const request: Schemas['SessionActivityRequest'] = { expectedSessionId };
		return this.mutateCookies(async () => {
			const response = await apiClient.post<SessionState>('/api/auth/activity', request, options);
			return requireApiData(response, 'ยังต่อเวลาการใช้งานไม่ได้ กรุณาลองอีกครั้ง');
		});
	}

	async revokeSession(sessionId: string, options: { current?: boolean } = {}): Promise<void> {
		const intent = authStore.beginAuthChange();
		const epoch = authStore.sessionEpoch;
		await this.mutateCookies(async () => {
			if (!(await this.refreshMutationSecurity(intent, epoch))) return;
			if (options.current) await notificationStore.prepareLogout();
			if (!this.currentIntent(intent, epoch)) return;
			const response = await apiClient.delete<EmptyData>(`/api/auth/sessions/${sessionId}`);
			requireApiData(response, 'ไม่สามารถเพิกถอนเซสชันได้');
			if (options.current === true && this.currentIntent(intent, epoch)) {
				clearSessionSecurity();
				authStore.clearUser();
			}
		});
	}

	async logoutAll(): Promise<void> {
		const intent = authStore.beginAuthChange();
		const epoch = authStore.sessionEpoch;
		await this.mutateCookies(async () => {
			if (!(await this.refreshMutationSecurity(intent, epoch))) return;
			await notificationStore.prepareLogout();
			if (!this.currentIntent(intent, epoch)) return;
			const response = await apiClient.post<EmptyData>('/api/auth/logout-all');
			requireApiData(response, 'ไม่สามารถออกจากระบบทุกอุปกรณ์ได้');
			if (!this.currentIntent(intent, epoch)) return;
			clearSessionSecurity();
			authStore.clearUser();
		});
	}

	async refreshCurrentUser(
		options: { silent?: boolean; invalidate?: boolean } = {}
	): Promise<AuthRefreshResult> {
		if (options.invalidate) this.refreshVersion++;
		const generation = authStore.generation;
		const silent = options.silent ?? true;
		if (!silent) authStore.setLoading(true);
		if (this.refreshInFlight?.generation === generation) return this.refreshInFlight.promise;
		// Explicit actions own cookie selection. Reconcile after them instead of reading old cookies.
		if (this.cookieMutation) {
			await this.cookieMutation;
			return this.refreshCurrentUser({ ...options, invalidate: false });
		}
		const promise = this.performRefresh(generation);
		const flight = { generation, promise };
		this.refreshInFlight = flight;
		try {
			return await promise;
		} finally {
			if (this.refreshInFlight === flight) this.refreshInFlight = null;
			if (generation === authStore.generation) authStore.setLoading(false);
		}
	}

	private async performRefresh(generation: number): Promise<AuthRefreshResult> {
		while (true) {
			const version = this.refreshVersion;
			try {
				let response = await apiClient.get<CurrentUserDto>('/api/auth/me');
				if (generation !== authStore.generation) return this.supersededRefresh();
				if (version !== this.refreshVersion) continue;
				if (response.status === 401) {
					const restored = await this.mutateCookies(async () => {
						if (generation !== authStore.generation || version !== this.refreshVersion) return null;
						// Deliberate empty JSON is required by the restore origin boundary.
						return apiClient.post<LoginData>('/api/auth/restore', {});
					});
					if (generation !== authStore.generation) return this.supersededRefresh();
					if (version !== this.refreshVersion || !restored) continue;
					// Another tab may already have a live session; it takes precedence over the grant.
					response =
						restored.status === 409
							? await apiClient.get<CurrentUserDto>('/api/auth/me')
							: { ...restored, data: restored.data?.user };
					if (generation !== authStore.generation) return this.supersededRefresh();
					if (version !== this.refreshVersion) continue;
				}
				const decision = authRefreshDecision(response.status);

				if (decision.result === 'authenticated') {
					if (!response.success || response.data === undefined) {
						authStore.setUnavailable();
						return 'unavailable';
					}
					const currentUser = normalizeCurrentUser(response.data);
					authStore.setUser(currentUser.user, currentUser.permissions, currentUser.session);
					announceAuthenticatedSession(currentUser.session);
					return 'authenticated';
				}

				if (decision.clear) {
					clearSessionSecurity();
					authStore.clearUser();
				} else {
					authStore.setUnavailable();
				}
				return decision.result;
			} catch {
				if (generation !== authStore.generation) return this.supersededRefresh();
				if (version !== this.refreshVersion) continue;
				authStore.setUnavailable();
				return 'unavailable';
			}
		}
	}

	/**
	 * Get full user profile with all fields
	 */
	async getFullProfile(options: ApiRequestOptions = {}): Promise<ProfileResponse> {
		const response = await apiClient.get<ProfileResponse>('/api/auth/me/profile', options);
		return requireApiData(response, 'ไม่สามารถโหลดข้อมูลได้');
	}

	/**
	 * Update user profile
	 */
	async updateProfile(data: UpdateProfileRequestDto): Promise<ProfileResponse> {
		const response = await apiClient.put<ProfileResponse>('/api/auth/me/profile', data);
		return requireApiData(response, 'ไม่สามารถบันทึกข้อมูลได้');
	}

	/**
	 * Change password
	 */
	async changePassword(
		data: ChangePasswordRequestDto
	): Promise<{ success: boolean; message: string }> {
		const intent = authStore.beginAuthChange();
		const epoch = authStore.sessionEpoch;
		return this.mutateCookies(async () => {
			if (!(await this.refreshMutationSecurity(intent, epoch)))
				throw new ApiClientError('การเข้าสู่ระบบเปลี่ยนแปลง กรุณาลองอีกครั้ง', 409);
			const response = await apiClient.post<EmptyData>('/api/auth/me/change-password', data);
			requireApiData(response, 'ไม่สามารถเปลี่ยนรหัสผ่านได้');
			return { success: true, message: response.message || 'เปลี่ยนรหัสผ่านสำเร็จ' };
		});
	}
}

export const authAPI = new AuthAPI();
