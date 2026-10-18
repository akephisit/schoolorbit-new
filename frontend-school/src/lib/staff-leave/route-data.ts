import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';

export interface LeaveOwnedRead<T> {
	identity: string;
	value: T;
}

export function captureLeaveRead<T>(
	allowed: () => boolean,
	read: () => Promise<T>,
	fallback: T
): Promise<RouteLoadResult<LeaveOwnedRead<T>>> {
	return captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => {
			const identity = appIdentityKey();
			return {
				identity,
				value: user?.user_type === 'staff' && allowed() ? await read() : fallback
			};
		}),
		'โหลดข้อมูลไม่สำเร็จ'
	);
}

export async function leaveSource<T>(
	source: Promise<RouteLoadResult<LeaveOwnedRead<T>>>,
	identity: string
): Promise<RouteLoadResult<T>> {
	const result = await source;
	if (!result.ok) return result;
	if (!identity || identity !== result.data.identity)
		return { ok: false, data: null, error: 'บริบทผู้ใช้เปลี่ยนแปลง กรุณาเปิดหน้านี้อีกครั้ง' };
	return { ok: true, data: result.data.value, error: null };
}
