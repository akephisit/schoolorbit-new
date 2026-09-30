import { get } from 'svelte/store';
import { authStore } from '$lib/stores/auth';
import { can } from '$lib/stores/permissions';

export function waitForSupervisionAccess(permissions: string[]): Promise<boolean> {
	const allowed = () => get(authStore).isAuthenticated && get(can).hasAny(...permissions);
	if (!get(authStore).isLoading) return Promise.resolve(allowed());
	return new Promise((resolve) => {
		const unsubscribe = authStore.subscribe((state) => {
			if (state.isLoading) return;
			queueMicrotask(() => {
				unsubscribe();
				resolve(allowed());
			});
		});
	});
}
