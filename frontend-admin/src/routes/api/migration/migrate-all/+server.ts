import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';

const BACKEND_SCHOOL_URL = env.BACKEND_SCHOOL_URL || 'http://localhost:8081';
const INTERNAL_SECRET = env.INTERNAL_API_SECRET || '';

export const POST: RequestHandler = async () => {
    try {
        const response = await fetch(`${BACKEND_SCHOOL_URL}/internal/migrate-all`, {
            method: 'POST',
            headers: {
                'X-Internal-Secret': INTERNAL_SECRET,
                'Content-Type': 'application/json'
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
        console.error('Migrate all error:', error);
        return new Response(
            JSON.stringify({ error: 'Failed to migrate schools' }),
            {
                status: 500,
                headers: { 'Content-Type': 'application/json' }
            }
        );
    }
};
