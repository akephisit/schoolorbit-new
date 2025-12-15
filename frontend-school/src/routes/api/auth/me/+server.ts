import { env } from '$env/dynamic/private';

const BACKEND_URL = env.BACKEND_SCHOOL_URL || 'https://school-api.schoolorbit.app';


/** @type {import('./$types').RequestHandler} */
export async function GET({ cookies, request }) {
    try {
        const authToken = cookies.get('auth_token');

        if (!authToken) {
            return new Response(
                JSON.stringify({
                    error: 'ไม่พบข้อมูลผู้ใช้'
                }),
                {
                    status: 401,
                    headers: {
                        'Content-Type': 'application/json'
                    }
                }
            );
        }

        // Backend extracts subdomain from Origin header
        const response = await fetch(`${BACKEND_URL}/api/auth/me`, {
            headers: {
                Cookie: `auth_token=${authToken}`
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
        console.error('Me proxy error:', error);
        return new Response(
            JSON.stringify({
                error: 'เกิดข้อผิดพลาด'
            }),
            {
                status: 500,
                headers: {
                    'Content-Type': 'application/json'
                }
            }
        );
    }
}
