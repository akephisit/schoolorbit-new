<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { authAPI } from '$lib/api/auth';
	import { authStore } from '$lib/stores/auth';
	import { page } from '$app/state';
	import type { Snippet } from 'svelte';

	let { children }: { children?: Snippet } = $props();

	let loading = $state(true);
	let authorized = $state(false);

	onMount(async () => {
		// Check authentication
		const isAuth = await authAPI.checkAuth();
		if (!isAuth) {
			// Not authenticated - save current URL and redirect to login
			const currentPath = page.url.pathname + page.url.search;
			sessionStorage.setItem('redirectAfterLogin', currentPath);
			goto(resolve('/login'));
			return;
		}

		// Get user from store
		const user = $authStore.user;
		
		// Check if user is staff (not student)
		if (user && user.user_type === 'student') {
			// Student trying to access staff area - redirect to student portal
			goto(resolve('/student'));
			return;
		}

		// User is authorized
		authorized = true;
		loading = false;
	});
</script>

{#if loading}
	<div class="min-h-screen flex items-center justify-center">
		<div class="text-center">
			<div
				class="w-16 h-16 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"
			></div>
			<p class="text-muted-foreground">กำลังโหลด...</p>
		</div>
	</div>
{:else if authorized}
	{@render children?.()}
{/if}
