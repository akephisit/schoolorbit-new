import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';

const BACKEND_SCHOOL_URL = env.BACKEND_SCHOOL_URL || 'http://localhost:8081';
const INTERNAL_SECRET = env.INTERNAL_API_SECRET || '';

export const POST: RequestHandler = async () => {
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
        console.log(`📡 Triggering migration for all schools at ${BACKEND_SCHOOL_URL}/internal/migrate-all`);

        const response = await fetch(`${BACKEND_SCHOOL_URL}/internal/migrate-all`, {
            method: 'POST',
            headers: {
                'X-Internal-Secret': INTERNAL_SECRET,
                'Content-Type': 'application/json'
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
        console.error('❌ Migrate all error:', error);
        return new Response(
            JSON.stringify({
                error: 'Failed to migrate schools',
                details: error instanceof Error ? error.message : 'Unknown error'
            }),
            {
                status: 500,
                headers: { 'Content-Type': 'application/json' }
            }
        );
    }
};
