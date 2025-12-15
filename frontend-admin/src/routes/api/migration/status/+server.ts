import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';

const BACKEND_SCHOOL_URL = env.BACKEND_SCHOOL_URL || 'http://localhost:8081';
const INTERNAL_SECRET = env.INTERNAL_API_SECRET || '';

export const GET: RequestHandler = async () => {
    try {
        const response = await fetch(`${BACKEND_SCHOOL_URL}/internal/migration-status`, {
            headers: {
                'X-Internal-Secret': INTERNAL_SECRET
            }
        });

        const data = await response.json();

        return new Response(JSON.stringify(data), {
            status: response.status,
            headers: {
                'Content-Type': 'application/json'
            }
        });
    } catch (error) {
        console.error('Migration status error:', error);
        return new Response(
            JSON.stringify({ error: 'Failed to fetch migration status' }),
            {
                status: 500,
                headers: { 'Content-Type': 'application/json' }
            }
        );
    }
};
