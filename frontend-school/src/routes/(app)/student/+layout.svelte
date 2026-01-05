<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { authAPI } from '$lib/api/auth';
	import { authStore } from '$lib/stores/auth';
	import type { Snippet } from 'svelte';

	let { children }: { children?: Snippet } = $props();

	let user = $state<any>(null);
	let loading = $state(true);

	onMount(async () => {
		// Check authentication
		const isAuth = await authAPI.checkAuth();
		if (!isAuth) {
			goto(resolve('/login'));
			return;
		}

		// Subscribe to authStore to get user
		authStore.subscribe((state) => {
			user = state.user;
			
			// Check if user is a student
			if (user && user.user_type !== 'student') {
				// Redirect non-students to staff dashboard
				goto(resolve('/staff'));
			}
		});

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
{:else if user}
	<div class="min-h-screen bg-background">
		<!-- Student Portal Content -->
		{@render children?.()}
	</div>
{/if}
