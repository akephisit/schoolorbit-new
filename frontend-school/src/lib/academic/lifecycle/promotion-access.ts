import { get } from 'svelte/store';
import { authStore } from '#lib/stores/auth.js';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';

/** Wait for the app layout's current-user refresh before starting promotion reads. */
export function waitForPromotionReadAccess(): Promise<boolean> {
	const current = get(authStore);
	if (!current.isLoading)
		return Promise.resolve(
			current.isAuthenticated && get(can).has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL)
		);
	return new Promise((resolve) => {
		const unsubscribe = authStore.subscribe((state) => {
			if (state.isLoading) return;
			queueMicrotask(() => {
				unsubscribe();
				resolve(state.isAuthenticated && get(can).has(PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL));
			});
		});
	});
}
