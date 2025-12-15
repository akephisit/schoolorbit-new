import { env } from '$env/dynamic/private';

const BACKEND_URL = env.BACKEND_SCHOOL_URL || 'https://school-api.schoolorbit.app';


/** @type {import('./$types').RequestHandler} */
export async function POST({ request, cookies }) {
    try {
        const body = await request.json();

        // Extract subdomain from request hostname
        const host = request.headers.get('host') || '';
        const subdomain = host.split('.')[0]; // e.g., "school1" from "school1.schoolorbit.app"

        console.log('Login request for subdomain:', subdomain);

        const response = await fetch(`${BACKEND_URL}/api/auth/login`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-School-Subdomain': subdomain // Forward subdomain to backend
            },
            body: JSON.stringify(body)
        });

        const data = await response.json();

        // Forward cookies from backend to client
        const setCookieHeader = response.headers.get('set-cookie');
        if (setCookieHeader) {
            // Parse and set cookie
            const [cookiePair] = setCookieHeader.split(';');
            const [name, value] = cookiePair.split('=');

            cookies.set(name, value, {
                path: '/',
                httpOnly: true,
                secure: false, // Set true in production
                sameSite: 'lax',
                maxAge: 60 * 60 * 24 * 7 // 7 days
            });
        }

        return new Response(JSON.stringify(data), {
            status: response.status,
            headers: {
                'Content-Type': 'application/json'
            }
        });
    } catch (error) {
        console.error('Login proxy error:', error);
        return new Response(
            JSON.stringify({
                error: 'เกิดข้อผิดพลาดในการเข้าสู่ระบบ'
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
