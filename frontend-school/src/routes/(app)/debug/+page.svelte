<script lang="ts">
	import { authStore } from '$lib/stores/auth';
	import { userPermissions } from '$lib/stores/permissions';
	import { PageShell } from '$lib/components/app-layout';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { authAPI } from '$lib/api/auth';
	import { toast } from 'svelte-sonner';

	const authState = $derived($authStore);
	const permissions = $derived($userPermissions);

	async function forceRefresh() {
		const result = await authAPI.refreshCurrentUser({ silent: false });
		if (result === 'authenticated') {
			window.location.reload();
		} else if (result === 'unavailable') {
			toast.warning('ระบบยืนยันตัวตนไม่พร้อมใช้งาน กรุณาลองใหม่อีกครั้ง');
		}
	}
</script>

<PageShell title="Debug: Auth State" description="ตรวจสอบ session และ permission ของผู้ใช้ปัจจุบัน">
	{#snippet actions()}
		<Button onclick={forceRefresh}>Force Refresh Auth</Button>
	{/snippet}

	<Card class="border-yellow-500">
		<CardHeader>
			<CardTitle>Raw Permissions Value</CardTitle>
		</CardHeader>
		<CardContent class="space-y-2">
			<p><strong>Type:</strong> <code>{typeof permissions}</code></p>
			<p><strong>Is Array:</strong> <code>{Array.isArray(permissions)}</code></p>
			<p>
				<strong>Length:</strong>
				<code>{permissions.length}</code>
			</p>
			<p><strong>Value:</strong></p>
			<pre class="bg-slate-100 dark:bg-slate-800 p-4 rounded overflow-auto text-xs">{JSON.stringify(
					permissions,
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
				{#if permissions.length === 0}
					<p class="text-orange-500">⚠️ Permissions array is empty!</p>
				{:else}
					<p class="text-green-600">✅ Found {permissions.length} permissions:</p>
					<ul class="list-disc pl-5 max-h-96 overflow-auto">
						{#each permissions as perm (perm)}
							<li class="text-sm font-mono">{perm}</li>
						{/each}
					</ul>
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
					canCreateOwn: {permissions.includes('achievement.create.own') ? '✅ TRUE' : '❌ FALSE'}
				</p>
				<p>
					canCreateAll: {permissions.includes('achievement.create.all') ? '✅ TRUE' : '❌ FALSE'}
				</p>
				<p>
					canReadOwn: {permissions.includes('achievement.read.own') ? '✅ TRUE' : '❌ FALSE'}
				</p>
				<p>
					canReadAll: {permissions.includes('achievement.read.all') ? '✅ TRUE' : '❌ FALSE'}
				</p>
			</div>
		</CardContent>
	</Card>
</PageShell>
