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

export interface ConsentRecord {
    id: string;
    user_id: string;
    user_type: string;
    consent_type: string;
    consent_type_name: string | null;
    purpose: string;
    data_categories: string[];
    consent_status: 'pending' | 'granted' | 'denied' | 'withdrawn';
    granted_at: string | null;
    withdrawn_at: string | null;
    expires_at: string | null;
    is_expired: boolean;
    is_required: boolean;
    consent_method: string;
    is_minor_consent: boolean;
    parent_guardian_name: string | null;
    created_at: string;
}

export interface UserConsentStatus {
    user_id: string;
    user_type: string;
    total_required: number;
    granted_required: number;
    is_compliant: boolean;
    missing_required_consents: string[];
    consents: ConsentRecord[];
}

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

const API_BASE = '/api';

export const consentApi = {
    /**
     * Get all consent types for a user type
     */
    async getConsentTypes(userType: string = 'student'): Promise<ConsentType[]> {
        const response = await fetch(`${API_BASE}/consent/types`, {
            headers: {
                'user-type': userType
            }
        });

        if (!response.ok) {
            throw new Error('Failed to fetch consent types');
        }

        return response.json();
    },

    /**
     * Get current user's consent status
     */
    async getMyConsentStatus(): Promise<UserConsentStatus> {
        const response = await fetch(`${API_BASE}/consent/my-status`, {
            credentials: 'include'
        });

        if (!response.ok) {
            throw new Error('Failed to fetch consent status');
        }

        return response.json();
    },

    /**
     * Give consent
     */
    async giveConsent(request: CreateConsentRequest): Promise<{ success: boolean; message: string; consent_id: string }> {
        const response = await fetch(`${API_BASE}/consent`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            credentials: 'include',
            body: JSON.stringify(request)
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to give consent');
        }

        return response.json();
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
    async withdrawConsent(consentId: string, reason?: string): Promise<{ success: boolean; message: string }> {
        const response = await fetch(`${API_BASE}/consent/${consentId}/withdraw`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            credentials: 'include',
            body: JSON.stringify({ reason })
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.error || 'Failed to withdraw consent');
        }

        return response.json();
    },

    /**
     * Get consent summary (Admin only)
     */
    async getConsentSummary(): Promise<ConsentSummary> {
        const response = await fetch(`${API_BASE}/consent/summary`, {
            credentials: 'include'
        });

        if (!response.ok) {
            throw new Error('Failed to fetch consent summary');
        }

        return response.json();
    }
};
