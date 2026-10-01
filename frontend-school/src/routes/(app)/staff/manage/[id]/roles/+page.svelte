<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { PageShell } from '$lib/components/app-layout';
	import { Button } from '$lib/components/ui/button';
	import StaffBreadcrumb from '$lib/components/staff/StaffBreadcrumb.svelte';
	import { staffReturnHref, withStaffReturn } from '$lib/navigation/staff-management';
	import { LatestRequest } from '$lib/async/latest-request';
	import { getStaffProfile, type StaffProfileResponse } from '$lib/api/staff';
	import { requireApiData } from '$lib/api/client';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import UserRoleManager from '$lib/components/UserRoleManager.svelte';

	let { data }: PageProps = $props();
	const userId = $derived(data.userId);
	const returnHref = $derived(staffReturnHref(page.url));
	const profileHref = $derived(withStaffReturn(`/staff/manage/${userId}`, returnHref));
	let staff: StaffProfileResponse | null = $state(null);
	let identityError = $state('');
	let identityLoading = $state(true);
	const identityRequest = new LatestRequest();
	$effect.pre(() => {
		const source = data.staff;
		untrack(() => {
			staff = null;
			identityError = '';
			identityLoading = true;
			const ticket = identityRequest.begin();
			void source.then((result) => {
				if (!identityRequest.isCurrent(ticket.revision)) return;
				identityLoading = false;
				if (result.ok) staff = result.data;
				else identityError = result.error;
			});
		});
		return () => identityRequest.abort();
	});
	async function retryIdentity() {
		const ticket = identityRequest.begin();
		identityLoading = true;
		identityError = '';
		const result = await captureRouteLoad(
			getStaffProfile(userId, { signal: ticket.signal }).then((response) =>
				requireApiData(response, 'โหลดชื่อบุคลากรไม่สำเร็จ')
			),
			'โหลดชื่อบุคลากรไม่สำเร็จ'
		);
		if (!identityRequest.isCurrent(ticket.revision)) return;
		identityLoading = false;
		if (result.ok) staff = result.data;
		else identityError = result.error;
	}
	onDestroy(() => identityRequest.abort());
</script>

<PageShell
	title="บทบาทและสิทธิ์บุคลากร"
	description={staff
		? `${staff.first_name} ${staff.last_name} • ${staff.username}`
		: `บัญชีบุคลากร ${userId}`}
	backHref={profileHref}
	backLabel="กลับข้อมูลบุคลากร"
	backPreload="tap"
>
	{#snippet meta()}<StaffBreadcrumb
			{returnHref}
			name={staff ? `${staff.first_name} ${staff.last_name}` : undefined}
			{profileHref}
			current="บทบาทและสิทธิ์"
		/>{/snippet}
	{#snippet actions()}<Button href={returnHref} data-sveltekit-preload-data="off" variant="outline"
			>กลับรายชื่อบุคลากร</Button
		>{/snippet}
	<div data-testid="staff-role-identity" aria-busy={identityLoading}>
		{#if identityLoading}<Skeleton class="h-4 w-56" />
		{:else if identityError}<div class="flex flex-wrap items-center gap-2">
				<p role="status" class="text-sm text-muted-foreground">{identityError}</p>
				<Button variant="outline" size="sm" onclick={retryIdentity}>ลองโหลดชื่ออีกครั้ง</Button>
			</div>{/if}
	</div>
	<UserRoleManager {userId} rolesRead={data.roles} permissionsRead={data.permissions} />
</PageShell>
