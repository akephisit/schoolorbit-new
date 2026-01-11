<script lang="ts">
	import { authStore } from '$lib/stores/auth';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';

	const authState = $derived($authStore);
</script>

<div class="container mx-auto p-8 space-y-6">
	<h1 class="text-3xl font-bold">Debug: Auth State</h1>

	<Card>
		<CardHeader>
			<CardTitle>Current User</CardTitle>
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
			<CardTitle>Permissions Check</CardTitle>
		</CardHeader>
		<CardContent>
			<div class="space-y-2">
				<p><strong>Permissions Array:</strong></p>
				{#if authState.user?.permissions}
					<ul class="list-disc pl-5">
						{#each authState.user.permissions as perm}
							<li class="text-sm font-mono">{perm}</li>
						{/each}
					</ul>
				{:else}
					<p class="text-red-500">⚠️ No permissions found!</p>
				{/if}

				<hr class="my-4" />

				<p><strong>Achievement Permissions:</strong></p>
				<ul class="space-y-1 text-sm">
					<li>
						achievement.create.own: {authState.user?.permissions?.includes('achievement.create.own')
							? '✅'
							: '❌'}
					</li>
					<li>
						achievement.create.all: {authState.user?.permissions?.includes('achievement.create.all')
							? '✅'
							: '❌'}
					</li>
					<li>
						achievement.read.own: {authState.user?.permissions?.includes('achievement.read.own')
							? '✅'
							: '❌'}
					</li>
					<li>
						achievement.read.all: {authState.user?.permissions?.includes('achievement.read.all')
							? '✅'
							: '❌'}
					</li>
					<li>
						achievement.update.own: {authState.user?.permissions?.includes('achievement.update.own')
							? '✅'
							: '❌'}
					</li>
					<li>
						achievement.update.all: {authState.user?.permissions?.includes('achievement.update.all')
							? '✅'
							: '❌'}
					</li>
					<li>
						achievement.delete.own: {authState.user?.permissions?.includes('achievement.delete.own')
							? '✅'
							: '❌'}
					</li>
					<li>
						achievement.delete.all: {authState.user?.permissions?.includes('achievement.delete.all')
							? '✅'
							: '❌'}
					</li>
				</ul>
			</div>
		</CardContent>
	</Card>
</div>
