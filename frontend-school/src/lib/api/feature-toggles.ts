/**
 * Feature Toggles API Client
 * Module-based permission control for managing feature flags
 */

import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

export interface FeatureToggle {
    id: string;
    code: string;
    name: string;
    name_en: string | null;
    module: string | null;
    is_enabled: boolean;
}

export interface FeatureListResponse {
    success: boolean;
    data: FeatureToggle[];
}

export interface FeatureToggleResponse {
    success: boolean;
    data: FeatureToggle | null;
    message: string | null;
}

export interface UpdateFeatureRequest {
    is_enabled?: boolean;
}

/**
 * List all feature toggles (filtered by user's module permissions)
 */
export async function listFeatures(): Promise<FeatureToggle[]> {
    const response = await fetch(`${BACKEND_URL}/api/admin/features`, {
        credentials: 'include'
    });

    if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Failed to fetch features' }));
        throw new Error(error.error || 'Failed to fetch features');
    }

    const result: FeatureListResponse = await response.json();
    return result.data;
}

/**
 * Get single feature toggle
 */
export async function getFeature(id: string): Promise<FeatureToggle> {
    const response = await fetch(`${BACKEND_URL}/api/admin/features/${id}`, {
        credentials: 'include'
    });

    if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Feature not found' }));
        throw new Error(error.error || 'Feature not found');
    }

    const result: FeatureToggleResponse = await response.json();
    if (!result.data) {
        throw new Error('Feature not found');
    }
    return result.data;
}

/**
 * Update feature toggle
 */
export async function updateFeature(
    id: string,
    data: UpdateFeatureRequest
): Promise<FeatureToggle> {
    const response = await fetch(`${BACKEND_URL}/api/admin/features/${id}`, {
        method: 'PUT',
        headers: {
            'Content-Type': 'application/json'
        },
        credentials: 'include',
        body: JSON.stringify(data)
    });

    if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Failed to update feature' }));
        throw new Error(error.error || 'Failed to update feature');
    }

    const result: FeatureToggleResponse = await response.json();
    if (!result.data) {
        throw new Error('Failed to update feature');
    }
    return result.data;
}

/**
 * Quick toggle feature on/off
 */
export async function toggleFeature(id: string): Promise<FeatureToggle> {
    const response = await fetch(`${BACKEND_URL}/api/admin/features/${id}/toggle`, {
        method: 'POST',
        credentials: 'include'
    });

    if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Failed to toggle feature' }));
        throw new Error(error.error || 'Failed to toggle feature');
    }

    const result: FeatureToggleResponse = await response.json();
    if (!result.data) {
        throw new Error('Failed to toggle feature');
    }
    return result.data;
}
