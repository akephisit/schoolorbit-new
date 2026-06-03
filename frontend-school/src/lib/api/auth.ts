import { apiClient, requireApiData } from '$lib/api/client';
import { authStore, type User } from '$lib/stores/auth';
import { toast } from 'svelte-sonner';

export interface LoginRequest {
	username: string;
	password: string;
	rememberMe?: boolean;
}

export interface ProfileResponse {
	// Read-only fields
	id: string;
	username?: string;
	nationalId?: string;
	firstName: string;
	lastName: string;
	userType: string;
	status: string;
	createdAt: string;
	updatedAt: string;
	primaryRoleName?: string;

	// Editable fields
	title?: string;
	nickname?: string;
	email?: string;
	phone?: string;
	emergencyContact?: string;
	lineId?: string;
	dateOfBirth?: string;
	gender?: string;
	address?: string;
	profileImageUrl?: string;
	hiredDate?: string;
}

interface BackendUser extends Omit<User, 'role' | 'user_type'> {
	role?: string;
	userType?: string;
	user_type?: string;
}

interface LoginData {
	user: BackendUser;
}

function normalizeUser(userData: BackendUser): User {
	const userType = userData.userType || userData.user_type;

	return {
		...userData,
		role: userData.role || userData.primaryRoleName || userType || '',
		user_type: userType
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
			if (!response.success || !response.data?.user) {
				throw new Error(response.error || 'เกิดข้อผิดพลาดในการเข้าสู่ระบบ');
			}
			const user = normalizeUser(response.data.user);

			authStore.setUser(user);
			toast.success(response.message || 'เข้าสู่ระบบสำเร็จ');

			return user;
		} catch (error: unknown) {
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
		try {
			const response = await apiClient.post<Record<string, never>>('/api/auth/logout');
			if (!response.success) throw new Error(response.error || 'ออกจากระบบไม่สำเร็จ');
			toast.success(response.message || 'ออกจากระบบสำเร็จ');
		} catch (error) {
			console.error('Logout error:', error);
		} finally {
			authStore.clearUser();
		}
	}

	/**
	 * Check authentication status - Direct to backend through the shared client-side API wrapper.
	 */
	async checkAuth(): Promise<boolean> {
		authStore.setLoading(true);

		try {
			const response = await apiClient.get<BackendUser>('/api/auth/me');
			const userData = requireApiData(response, 'Failed to check auth');
			const user = normalizeUser(userData);

			authStore.setUser(user);
			return true;
		} catch {
			// Only log actual errors, not 401/403 which are expected state checks
			// Or just suppress log for auth check entirely to keep console clean
			// console.error('Auth check error:', error);
			authStore.clearUser();
			return false;
		} finally {
			authStore.setLoading(false);
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
	async updateProfile(data: {
		title?: string;
		nickname?: string;
		email?: string;
		phone?: string;
		emergencyContact?: string;
		lineId?: string;
		dateOfBirth?: string;
		gender?: string;
		address?: string;
		profileImageUrl?: string;
	}): Promise<ProfileResponse> {
		const response = await apiClient.put<ProfileResponse>('/api/auth/me/profile', data);
		return requireApiData(response, 'ไม่สามารถบันทึกข้อมูลได้');
	}

	/**
	 * Change password
	 */
	async changePassword(data: {
		currentPassword: string;
		newPassword: string;
	}): Promise<{ success: boolean; message: string }> {
		const response = await apiClient.post<Record<string, never>>(
			'/api/auth/me/change-password',
			data
		);
		if (!response.success) throw new Error(response.error || 'ไม่สามารถเปลี่ยนรหัสผ่านได้');

		return {
			success: true,
			message: response.message || 'เปลี่ยนรหัสผ่านสำเร็จ'
		};
	}
}

export const authAPI = new AuthAPI();
