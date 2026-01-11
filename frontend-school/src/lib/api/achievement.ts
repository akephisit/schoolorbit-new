import type { Achievement, CreateAchievementRequest, UpdateAchievementRequest, AchievementListFilter } from '$lib/types/achievement';
import { browser } from '$app/environment';

const API_BASE = '/api/achievements';

export async function getAchievements(filter?: AchievementListFilter): Promise<{ success: boolean; data?: Achievement[]; error?: string }> {
    try {
        const params = new URLSearchParams();
        if (filter?.user_id) params.append('user_id', filter.user_id);
        if (filter?.start_date) params.append('start_date', filter.start_date);
        if (filter?.end_date) params.append('end_date', filter.end_date);

        const res = await fetch(`${API_BASE}?${params.toString()}`);
        const data = await res.json();
        return data;
    } catch (e) {
        console.error('Fetch achievements error:', e);
        return { success: false, error: 'Network error' };
    }
}

export async function createAchievement(payload: CreateAchievementRequest): Promise<{ success: boolean; data?: Achievement; error?: string }> {
    try {
        const res = await fetch(API_BASE, {
            method: 'POST',
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

export async function updateAchievement(id: string, payload: UpdateAchievementRequest): Promise<{ success: boolean; data?: Achievement; error?: string }> {
    try {
        const res = await fetch(`${API_BASE}/${id}`, {
            method: 'PUT',
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

export async function deleteAchievement(id: string): Promise<{ success: boolean; error?: string }> {
    try {
        const res = await fetch(`${API_BASE}/${id}`, {
            method: 'DELETE'
        });
        const data = await res.json();
        return data;
    } catch (e) {
        console.error('Delete achievement error:', e);
        return { success: false, error: 'Network error' };
    }
}
