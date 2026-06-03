/**
 * Feature Toggles API Client
 * Module-based permission control for managing feature flags
 */

import { apiClient, requireApiData } from '$lib/api/client';

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
	const response = await apiClient.get<FeatureToggle[]>('/api/admin/features');
	return requireApiData(response, 'Failed to fetch features');
}

/**
 * Get single feature toggle
 */
export async function getFeature(id: string): Promise<FeatureToggle> {
	const response = await apiClient.get<FeatureToggle>(`/api/admin/features/${id}`);
	return requireApiData(response, 'Feature not found');
}

/**
 * Update feature toggle
 */
export async function updateFeature(
	id: string,
	data: UpdateFeatureRequest
): Promise<FeatureToggle> {
	const response = await apiClient.put<FeatureToggle>(`/api/admin/features/${id}`, data);
	return requireApiData(response, 'Failed to update feature');
}

/**
 * Quick toggle feature on/off
 */
export async function toggleFeature(id: string): Promise<FeatureToggle> {
	const response = await apiClient.post<FeatureToggle>(`/api/admin/features/${id}/toggle`);
	return requireApiData(response, 'Failed to toggle feature');
}
