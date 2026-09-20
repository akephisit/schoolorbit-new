export type RouteLoadResult<T> =
	{ ok: true; data: T; error: null } | { ok: false; data: null; error: string };

export async function captureRouteLoad<T>(
	operation: Promise<T>,
	fallbackMessage: string
): Promise<RouteLoadResult<T>> {
	try {
		return { ok: true, data: await operation, error: null };
	} catch (error) {
		return {
			ok: false,
			data: null,
			error: error instanceof Error && error.message ? error.message : fallbackMessage
		};
	}
}
