import { get } from 'svelte/store';
import { authStore, type User } from '$lib/stores/auth';
import { userPermissions } from '$lib/stores/permissions';

/** Reuse the layout's auth result; never start another current-user request. */
export function waitForAuthenticatedUser(): Promise<User | null> {
	const settled = () => (get(authStore).isAuthenticated ? get(authStore).user : null);
	if (!get(authStore).isLoading) return Promise.resolve(settled());
	return new Promise((resolve) => {
		const unsubscribe = authStore.subscribe((state) => {
			if (state.isLoading) return;
			queueMicrotask(() => {
				unsubscribe();
				resolve(settled());
			});
		});
	});
}

export function appIdentityKey(): string {
	const user = get(authStore).user;
	return user ? `${user.id}:${get(userPermissions).slice().sort().join(',')}` : '';
}
