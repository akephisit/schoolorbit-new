<script lang="ts">
	import { authStore } from '$lib/stores/auth';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { authAPI } from '$lib/api/auth';

	const authState = $derived($authStore);

	async function forceRefresh() {
		await authAPI.checkAuth();
		window.location.reload();
	}
</script>

<div class="container mx-auto p-8 space-y-6">
	<div class="flex items-center justify-between">
		<h1 class="text-3xl font-bold">Debug: Auth State</h1>
		<Button onclick={forceRefresh}>Force Refresh Auth</Button>
	</div>

	<Card class="border-yellow-500">
		<CardHeader>
			<CardTitle>Raw Permissions Value</CardTitle>
		</CardHeader>
		<CardContent class="space-y-2">
			<p><strong>Type:</strong> <code>{typeof authState.user?.permissions}</code></p>
			<p><strong>Is Array:</strong> <code>{Array.isArray(authState.user?.permissions)}</code></p>
			<p>
				<strong>Length:</strong>
				<code>{authState.user?.permissions?.length ?? 'N/A'}</code>
			</p>
			<p><strong>Value:</strong></p>
			<pre class="bg-slate-100 dark:bg-slate-800 p-4 rounded overflow-auto text-xs">{JSON.stringify(
					authState.user?.permissions,
					null,
					2
				)}</pre>
		</CardContent>
	</Card>

	<Card>
		<CardHeader>
			<CardTitle>Full User Object</CardTitle>
		</CardHeader>
		<CardContent>
			<pre class="bg-slate-100 dark:bg-slate-800 p-4 rounded overflow-auto text-xs">{JSON.stringify(
					authState.user,
					null,
					2
				)}</pre>
		</CardContent>
	</Card>

	<Card>
		<CardHeader>
			<CardTitle>Permissions List</CardTitle>
		</CardHeader>
		<CardContent>
			<div class="space-y-2">
				{#if authState.user?.permissions}
					{#if authState.user.permissions.length === 0}
						<p class="text-orange-500">⚠️ Permissions array is empty!</p>
					{:else}
						<p class="text-green-600">✅ Found {authState.user.permissions.length} permissions:</p>
						<ul class="list-disc pl-5 max-h-96 overflow-auto">
							{#each authState.user.permissions as perm}
								<li class="text-sm font-mono">{perm}</li>
							{/each}
						</ul>
					{/if}
				{:else}
					<p class="text-red-500">❌ No permissions field found! (null or undefined)</p>
				{/if}
			</div>
		</CardContent>
	</Card>

	<Card>
		<CardHeader>
			<CardTitle>Achievement Permissions Check</CardTitle>
		</CardHeader>
		<CardContent>
			<div class="space-y-1 text-sm font-mono">
				<p>
					canCreateOwn: {authState.user?.permissions?.includes('achievement.create.own')
						? '✅ TRUE'
						: '❌ FALSE'}
				</p>
				<p>
					canCreateAll: {authState.user?.permissions?.includes('achievement.create.all')
						? '✅ TRUE'
						: '❌ FALSE'}
				</p>
				<p>
					canReadOwn: {authState.user?.permissions?.includes('achievement.read.own')
						? '✅ TRUE'
						: '❌ FALSE'}
				</p>
				<p>
					canReadAll: {authState.user?.permissions?.includes('achievement.read.all')
						? '✅ TRUE'
						: '❌ FALSE'}
				</p>
			</div>
		</CardContent>
	</Card>
</div>
