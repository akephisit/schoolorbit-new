import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';

export interface BudgetOwnedRead<T> {
	identity: string;
	value: T;
}

export function budgetRead<T>(allowed: () => boolean, read: () => Promise<T>, fallback: T) {
	return captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => ({
			identity: appIdentityKey(),
			value: user?.user_type === 'staff' && allowed() ? await read() : fallback
		})),
		'โหลดข้อมูลงบประมาณไม่สำเร็จ'
	);
}

export async function budgetSource<T>(
	source: Promise<RouteLoadResult<BudgetOwnedRead<T>>>,
	identity: string
): Promise<RouteLoadResult<T>> {
	const result = await source;
	if (!result.ok) return result;
	if (!identity || identity !== result.data.identity)
		return { ok: false, data: null, error: 'บริบทผู้ใช้เปลี่ยนแปลง กรุณาเปิดหน้านี้อีกครั้ง' };
	return { ok: true, data: result.data.value, error: null };
}
