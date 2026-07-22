import type {
	Achievement,
	CreateAchievementRequest,
	UpdateAchievementRequest,
	AchievementListFilter
} from '$lib/types/achievement';
import { apiClient, type ApiResponse } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];
type EmptyData = Schemas['EmptyData'];

export async function getAchievements(
	filter?: AchievementListFilter
): Promise<ApiResponse<Achievement[]>> {
	try {
		const params = new URLSearchParams();
		if (filter?.user_id) params.append('user_id', filter.user_id);
		if (filter?.start_date) params.append('start_date', filter.start_date);
		if (filter?.end_date) params.append('end_date', filter.end_date);

		return await apiClient.get<Achievement[]>(`/api/achievements?${params.toString()}`);
	} catch (e) {
		console.error('Fetch achievements error:', e);
		return { success: false, error: 'Network error' };
	}
}

export async function createAchievement(
	payload: CreateAchievementRequest
): Promise<ApiResponse<Achievement>> {
	try {
		return await apiClient.post<Achievement>('/api/achievements', payload);
	} catch (e) {
		console.error('Create achievement error:', e);
		return { success: false, error: 'Network error' };
	}
}

export async function updateAchievement(
	id: string,
	payload: UpdateAchievementRequest
): Promise<ApiResponse<Achievement>> {
	try {
		return await apiClient.put<Achievement>(`/api/achievements/${id}`, payload);
	} catch (e) {
		console.error('Update achievement error:', e);
		return { success: false, error: 'Network error' };
	}
}

export async function deleteAchievement(id: string): Promise<ApiResponse<EmptyData>> {
	try {
		return await apiClient.delete<EmptyData>(`/api/achievements/${id}`);
	} catch (e) {
		console.error('Delete achievement error:', e);
		return { success: false, error: 'Network error' };
	}
}
