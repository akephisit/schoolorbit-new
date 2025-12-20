import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';

const BACKEND_SCHOOL_URL = env.BACKEND_SCHOOL_URL || 'http://localhost:8081';
const INTERNAL_SECRET = env.INTERNAL_API_SECRET || '';

export const GET: RequestHandler = async () => {
    // Validate environment variables
    if (!env.BACKEND_SCHOOL_URL) {
        console.error('❌ BACKEND_SCHOOL_URL is not configured');
        return new Response(
            JSON.stringify({
                error: 'Migration service not configured',
                details: 'BACKEND_SCHOOL_URL environment variable is missing'
            }),
            {
                status: 503,
                headers: { 'Content-Type': 'application/json' }
            }
        );
    }

    if (!env.INTERNAL_API_SECRET) {
        console.error('❌ INTERNAL_API_SECRET is not configured');
        return new Response(
            JSON.stringify({
                error: 'Migration service not configured',
                details: 'INTERNAL_API_SECRET environment variable is missing'
            }),
            {
                status: 503,
                headers: { 'Content-Type': 'application/json' }
            }
        );
    }

    try {
        console.log(`📡 Fetching migration status from ${BACKEND_SCHOOL_URL}/internal/migration-status`);

        const response = await fetch(`${BACKEND_SCHOOL_URL}/internal/migration-status`, {
            headers: {
                'X-Internal-Secret': INTERNAL_SECRET
            }
        });

        if (!response.ok) {
            console.error(`❌ Backend responded with status ${response.status}`);
            const errorText = await response.text();
            console.error(`Error details: ${errorText}`);
        }

        const data = await response.json();

        return new Response(JSON.stringify(data), {
            status: response.status,
            headers: {
                'Content-Type': 'application/json'
            }
        });
    } catch (error) {
        console.error('❌ Migration status error:', error);
        return new Response(
            JSON.stringify({
                error: 'Failed to fetch migration status',
                details: error instanceof Error ? error.message : 'Unknown error'
            }),
            {
                status: 500,
                headers: { 'Content-Type': 'application/json' }
            }
        );
    }
};
