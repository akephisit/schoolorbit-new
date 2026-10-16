import { authStore } from '#lib/stores/auth.svelte.js';
import { redirect } from '@sveltejs/kit';
import { resolve } from '$app/paths';
export const ssr = false;
export async function load() {
	await authStore.ready;
	if (!authStore.isAuthenticated) redirect(303, resolve('/login'));
	return {};
}
