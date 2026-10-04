import { get } from 'svelte/store';
import { authStore } from '#lib/stores/auth.js';
import { can } from '#lib/stores/permissions.js';

/** Wait for the app layout's current-user refresh before a route read. */
export function waitForAdmissionAccess(permission: string): Promise<boolean> {
	const current = get(authStore);
	if (!current.isLoading)
		return Promise.resolve(current.isAuthenticated && get(can).has(permission));
	return new Promise((resolve) => {
		const unsubscribe = authStore.subscribe((state) => {
			if (state.isLoading) return;
			queueMicrotask(() => {
				unsubscribe();
				resolve(state.isAuthenticated && get(can).has(permission));
			});
		});
	});
}
