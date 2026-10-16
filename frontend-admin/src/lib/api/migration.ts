import { apiClient } from '#lib/api/client.js';
// Migration API Client for frontend-admin

export interface MigrationResult {
	subdomain: string;
	status: string;
	version?: number;
	error?: string;
}

export interface MigrateAllResponse {
	total: number;
	success: number;
	failed: number;
	latest_version: number;
	results: MigrationResult[];
}

export interface SchoolMigrationStatus {
	subdomain: string;
	migration_version: number;
	migration_status: string;
	last_migrated_at?: string;
	migration_error?: string;
}

export interface MigrationStatusResponse {
	total_schools: number;
	migrated: number;
	pending: number;
	failed: number;
	outdated: number;
	active_pools: number;
	latest_version: number;
	schools: SchoolMigrationStatus[];
}

class MigrationAPI {
	private baseUrl = '/api/v1/operator/migration';

	/**
	 * Get migration status for all schools
	 */
	async getStatus(): Promise<MigrationStatusResponse> {
		return apiClient.raw(`${this.baseUrl}/status`);
	}
	async migrateAll(): Promise<MigrateAllResponse> {
		return apiClient.raw(`${this.baseUrl}/migrate-all`, { method: 'POST' });
	}
}

export const migrationAPI = new MigrationAPI();
