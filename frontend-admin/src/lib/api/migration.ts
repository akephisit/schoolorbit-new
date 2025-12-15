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
    private baseUrl = '/api/migration';

    /**
     * Get migration status for all schools
     */
    async getStatus(): Promise<MigrationStatusResponse> {
        const response = await fetch(`${this.baseUrl}/status`);
        if (!response.ok) {
            throw new Error('Failed to fetch migration status');
        }
        return response.json();
    }

    /**
     * Migrate all schools
     */
    async migrateAll(): Promise<MigrateAllResponse> {
        const response = await fetch(`${this.baseUrl}/migrate-all`, {
            method: 'POST'
        });
        if (!response.ok) {
            throw new Error('Failed to migrate schools');
        }
        return response.json();
    }

    /**
     * Migrate a single school
     */
    async migrateSchool(subdomain: string): Promise<MigrationResult> {
        const response = await fetch(`${this.baseUrl}/migrate/${subdomain}`, {
            method: 'POST'
        });
        if (!response.ok) {
            throw new Error(`Failed to migrate ${subdomain}`);
        }
        return response.json();
    }
}

export const migrationAPI = new MigrationAPI();
