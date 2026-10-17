import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';

interface OwnedRead<T> {
	identity: string;
	value: T;
}
export function captureCorrespondenceRead<T>(
	allowed: () => boolean,
	read: () => Promise<T>,
	fallback: T
): Promise<RouteLoadResult<OwnedRead<T>>> {
	return captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => {
			const identity = appIdentityKey();
			const value = user?.user_type === 'staff' && allowed() ? await read() : fallback;
			return { identity, value };
		}),
		'โหลดข้อมูลไม่สำเร็จ'
	);
}
export async function correspondenceSource<T>(
	source: Promise<RouteLoadResult<OwnedRead<T>>>,
	identity: string
): Promise<RouteLoadResult<T>> {
	const result = await source;
	if (!result.ok) return result;
	if (!identity || result.data.identity !== identity)
		return { ok: false, data: null, error: 'บริบทผู้ใช้งานเปลี่ยนแปลง กรุณาเปิดหน้านี้อีกครั้ง' };
	return { ok: true, data: result.data.value, error: null };
}
