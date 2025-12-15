import { env } from '$env/dynamic/private';

const BACKEND_URL = env.BACKEND_SCHOOL_URL || 'http://localhost:8081';

/** @type {import('./$types').RequestHandler} */
export async function POST({ cookies }) {
    // Clear auth cookie
    cookies.delete('auth_token', { path: '/' });

    try {
        await fetch(`${BACKEND_URL}/api/auth/logout`, {
            method: 'POST'
        });

        return new Response(
            JSON.stringify({
                success: true,
                message: 'ออกจากระบบสำเร็จ'
            }),
            {
                status: 200,
                headers: {
                    'Content-Type': 'application/json'
                }
            }
        );
    } catch (error) {
        return new Response(
            JSON.stringify({
                success: true,
                message: 'ออกจากระบบสำเร็จ'
            }),
            {
                status: 200,
                headers: {
                    'Content-Type': 'application/json'
                }
            }
        );
    }
}
