import type { Achievement, CreateAchievementRequest, UpdateAchievementRequest, AchievementListFilter } from '$lib/types/achievement';
import { PUBLIC_BACKEND_URL } from '$env/static/public';

const API_BASE_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

export interface ApiResponse<T> {
    success: boolean;
    data?: T;
    error?: string;
    message?: string;
}

export async function getAchievements(filter?: AchievementListFilter): Promise<ApiResponse<Achievement[]>> {
    try {
        const params = new URLSearchParams();
        if (filter?.user_id) params.append('user_id', filter.user_id);
        if (filter?.start_date) params.append('start_date', filter.start_date);
        if (filter?.end_date) params.append('end_date', filter.end_date);

        const res = await fetch(`${API_BASE_URL}/api/achievements?${params.toString()}`, {
            method: 'GET',
            credentials: 'include',
            headers: {
                'Content-Type': 'application/json'
            }
        });
        const data = await res.json();
        return data;
    } catch (e) {
        console.error('Fetch achievements error:', e);
        return { success: false, error: 'Network error' };
    }
}

export async function createAchievement(payload: CreateAchievementRequest): Promise<ApiResponse<Achievement>> {
    try {
        const res = await fetch(`${API_BASE_URL}/api/achievements`, {
            method: 'POST',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });
        const data = await res.json();
        return data;
    } catch (e) {
        console.error('Create achievement error:', e);
        return { success: false, error: 'Network error' };
    }
}

export async function updateAchievement(id: string, payload: UpdateAchievementRequest): Promise<ApiResponse<Achievement>> {
    try {
        const res = await fetch(`${API_BASE_URL}/api/achievements/${id}`, {
            method: 'PUT',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });
        const data = await res.json();
        return data;
    } catch (e) {
        console.error('Update achievement error:', e);
        return { success: false, error: 'Network error' };
    }
}

export async function deleteAchievement(id: string): Promise<ApiResponse<void>> {
    try {
        const res = await fetch(`${API_BASE_URL}/api/achievements/${id}`, {
            method: 'DELETE',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' }
        });
        const data = await res.json();
        return data;
    } catch (e) {
        console.error('Delete achievement error:', e);
        return { success: false, error: 'Network error' };
    }
}
