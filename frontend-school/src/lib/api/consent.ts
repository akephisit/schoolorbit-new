// ===================================================================
// Consent Management Types (PDPA Compliance)
// ===================================================================

export interface ConsentType {
	id: string;
	code: string;
	name: string;
	name_en: string | null;
	description: string | null;
	is_required: boolean;
	priority: number;
	applicable_user_types: string[];
	consent_text_template: string;
	consent_version: string;
	default_duration_days: number | null;
	is_active: boolean;
}

export type ConsentRecord = components['schemas']['ConsentRecordResponse'];
export type UserConsentStatus = components['schemas']['UserConsentStatus'];

export interface CreateConsentRequest {
	consent_type: string;
	consent_status: 'granted' | 'denied';
	is_minor_consent?: boolean;
	parent_guardian_name?: string;
	parent_relationship?: string;
}

export interface ConsentSummary {
	total_users: number;
	total_consents: number;
	granted: number;
	denied: number;
	withdrawn: number;
	pending: number;
	compliance_rate: number;
}

// ===================================================================
// Consent API Client
// ===================================================================

import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';

const API_BASE = '/api';

export const consentApi = {
	/**
	 * Get all consent types for a user type
	 */
	async getConsentTypes(userType: string = 'student'): Promise<ConsentType[]> {
		const response = await apiClient.get<ConsentType[]>(
			`${API_BASE}/consent/types?user_type=${encodeURIComponent(userType)}`
		);
		return requireApiData(response, 'Failed to fetch consent types');
	},

	/**
	 * Get current user's consent status
	 */
	async getMyConsentStatus(options: ApiRequestOptions = {}): Promise<UserConsentStatus> {
		const response = await apiClient.get<UserConsentStatus>(
			`${API_BASE}/consent/my-status`,
			options
		);
		return requireApiData(response, 'Failed to fetch consent status');
	},

	/**
	 * Give consent
	 */
	async giveConsent(
		request: CreateConsentRequest
	): Promise<{ success: boolean; message: string; consent_id: string }> {
		const response = await apiClient.post<{ consent_id: string }>(`${API_BASE}/consent`, request);
		const data = requireApiData(response, 'Failed to give consent');
		return {
			success: true,
			message: response.message || 'บันทึกความยินยอมสำเร็จ',
			consent_id: data.consent_id
		};
	},

	/**
	 * Give multiple consents at once
	 */
	async giveMultipleConsents(consents: CreateConsentRequest[]): Promise<void> {
		const promises = consents.map((consent) => this.giveConsent(consent));
		await Promise.all(promises);
	},

	/**
	 * Withdraw consent
	 */
	async withdrawConsent(consentId: string): Promise<{ success: boolean; message: string }> {
		const response = await apiClient.post<components['schemas']['EmptyData']>(
			`${API_BASE}/consent/${encodeURIComponent(consentId)}/withdraw`
		);
		if (!response.success) throw new Error(response.error || 'Failed to withdraw consent');
		return { success: true, message: response.message || 'ถอนความยินยอมสำเร็จ' };
	},

	/**
	 * Get consent summary (Admin only)
	 */
	async getConsentSummary(): Promise<ConsentSummary> {
		const response = await apiClient.get<ConsentSummary>(`${API_BASE}/consent/summary`);
		return requireApiData(response, 'Failed to fetch consent summary');
	}
};
