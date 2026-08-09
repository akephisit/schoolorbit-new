import { ApiClientError, apiClient, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';
import { clearSessionSecurity } from '$lib/api/session-security';
import { authStore, type User } from '$lib/stores/auth';
import { toast } from 'svelte-sonner';

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

function normalizeCurrentUser(userData: CurrentUserDto): User {
	return {
		id: userData.id,
		username: userData.username,
		firstName: userData.firstName,
		lastName: userData.lastName,
		role: userData.primaryRoleName ?? userData.userType,
		user_type: userData.userType,
		status: userData.status,
		primaryRoleName: userData.primaryRoleName ?? undefined,
		profileImageFileId: userData.profileImageFileId ?? undefined,
		permissions: userData.permissions
	};
}

class AuthAPI {
	/**
	 * Login - Direct to backend through the shared client-side API wrapper.
	 */
	async login(data: LoginRequest): Promise<User> {
		authStore.setLoading(true);

		try {
			const response = await apiClient.post<LoginData>('/api/auth/login', data);
			const user = normalizeCurrentUser(
				requireApiData(response, 'เกิดข้อผิดพลาดในการเข้าสู่ระบบ').user
			);

			authStore.setUser(user);
			toast.success(response.message || 'เข้าสู่ระบบสำเร็จ');

			return user;
		} catch (error: unknown) {
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
			authStore.setLoading(false);
		}
	}

	/**
	 * Logout - Direct to backend through the shared client-side API wrapper.
	 */
	async logout(): Promise<void> {
		const response = await apiClient.post<EmptyData>('/api/auth/logout');
		requireApiData(response, 'ออกจากระบบไม่สำเร็จ');
		clearSessionSecurity();
		authStore.clearUser();
		toast.success(response.message || 'ออกจากระบบสำเร็จ');
	}

	async listSessions(): Promise<SessionDto[]> {
		const response = await apiClient.get<SessionListData>('/api/auth/sessions');
		return requireApiData(response, 'ไม่สามารถโหลดรายการอุปกรณ์ได้').sessions;
	}

	async revokeSession(sessionId: string, options: { current?: boolean } = {}): Promise<void> {
		const response = await apiClient.delete<EmptyData>(`/api/auth/sessions/${sessionId}`);
		requireApiData(response, 'ไม่สามารถเพิกถอนเซสชันได้');
		if (options.current === true) {
			clearSessionSecurity();
			authStore.clearUser();
		}
	}

	async logoutAll(): Promise<void> {
		const response = await apiClient.post<EmptyData>('/api/auth/logout-all');
		requireApiData(response, 'ไม่สามารถออกจากระบบทุกอุปกรณ์ได้');
		clearSessionSecurity();
		authStore.clearUser();
	}

	/**
	 * Check authentication status - Direct to backend through the shared client-side API wrapper.
	 */
	async checkAuth(): Promise<boolean> {
		return this.refreshCurrentUser({ silent: false });
	}

	async refreshCurrentUser(options: { silent?: boolean } = {}): Promise<boolean> {
		const silent = options.silent ?? true;
		if (!silent) authStore.setLoading(true);
		try {
			const response = await apiClient.get<CurrentUserDto>('/api/auth/me');
			const userData = requireApiData(response, 'Failed to check auth');
			const user = normalizeCurrentUser(userData);

			authStore.setUser(user);
			return true;
		} catch {
			// Only log actual errors, not 401/403 which are expected state checks
			// Or just suppress log for auth check entirely to keep console clean
			// console.error('Auth check error:', error);
			authStore.clearUser();
			return false;
		} finally {
			if (!silent) authStore.setLoading(false);
		}
	}

	/**
	 * Get full user profile with all fields
	 */
	async getFullProfile(): Promise<ProfileResponse> {
		const response = await apiClient.get<ProfileResponse>('/api/auth/me/profile');
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
		const response = await apiClient.post<EmptyData>('/api/auth/me/change-password', data);
		if (!response.success) throw new Error(response.error || 'ไม่สามารถเปลี่ยนรหัสผ่านได้');

		return {
			success: true,
			message: response.message || 'เปลี่ยนรหัสผ่านสำเร็จ'
		};
	}
}

export const authAPI = new AuthAPI();
