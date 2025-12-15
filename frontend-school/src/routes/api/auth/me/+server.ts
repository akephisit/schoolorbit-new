import { env } from '$env/dynamic/private';

const BACKEND_URL = env.BACKEND_SCHOOL_URL || 'http://localhost:8081';

/** @type {import('./$types').RequestHandler} */
export async function GET({ cookies }) {
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
