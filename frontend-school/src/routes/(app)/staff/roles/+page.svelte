<script lang="ts">
	import { untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { requireApiData } from '#lib/api/client.js';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { roleAPI, type Role } from '#lib/api/roles.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import { Edit, Eye, Plus, Shield } from '@lucide/svelte';

	let { data }: PageProps = $props();
	const rolesRead = $derived(data.roles);
	const rolesRequest = new LatestRequest();
	let rolesLoaded = $state(false);
	let rolesError = $state('');
	let roles = $state<Role[]>([]);
	let loading = $state(true);

	const canReadRoles = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const canCreateRoles = $derived($can.has(PERMISSIONS.ROLES_CREATE_ALL));
	const canUpdateRoles = $derived($can.has(PERMISSIONS.ROLES_UPDATE_ALL));

	$effect.pre(() => {
		const source = rolesRead;
		untrack(() => {
			const ticket = rolesRequest.begin();
			loading = true;
			rolesError = '';
			void source.then((result) => applyRoles(result, ticket.revision));
		});
		return () => rolesRequest.abort();
	});
	function applyRoles(result: Awaited<typeof data.roles>, revision: number) {
		if (!rolesRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			roles = result.data.toSorted((a, b) => b.level - a.level);
			rolesLoaded = true;
		} else rolesError = result.error;
	}
	async function loadRoles() {
		if (!canReadRoles) return;
		const ticket = rolesRequest.begin();
		loading = true;
		rolesError = '';
		const result = await captureRouteLoad(
			roleAPI
				.listRoles({ include_inactive: true }, { signal: ticket.signal })
				.then((response) => requireApiData(response, 'โหลดบทบาทไม่สำเร็จ')),
			'โหลดบทบาทไม่สำเร็จ'
		);
		applyRoles(result, ticket.revision);
	}

	function getLevelBadgeColor(level: number): string {
		if (level >= 900) return 'bg-purple-500';
		if (level >= 80) return 'bg-red-500';
		if (level >= 50) return 'bg-orange-500';
		if (level >= 20) return 'bg-blue-500';
		return 'bg-gray-500';
	}

	function getUserTypeBadgeColor(userType: string): string {
		switch (userType) {
			case 'staff':
				return 'bg-blue-100 text-blue-800';
			case 'student':
				return 'bg-green-100 text-green-800';
			case 'parent':
				return 'bg-purple-100 text-purple-800';
			default:
				return 'bg-gray-100 text-gray-800';
		}
	}
</script>

<PageShell title="จัดการบทบาท" description="กำหนดบทบาทและสิทธิ์การเข้าถึงของผู้ใช้งาน">
	{#snippet actions()}
		<Button variant="outline" onclick={loadRoles} disabled={loading || !canReadRoles}>รีเฟรช</Button
		>
		{#if canCreateRoles}
			<Button onclick={() => goto(resolve('staff/roles/new'))} class="gap-2">
				<Plus class="h-4 w-4" />
				สร้างบทบาทใหม่
			</Button>
		{/if}
	{/snippet}

	<section data-testid="roles-list" aria-busy={loading}>
		{#if rolesError && rolesLoaded}<PageState
				title="อัปเดตบทบาทไม่สำเร็จ"
				description={rolesError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadRoles}
			/>{/if}
		{#if loading && rolesLoaded}<p role="status">กำลังอัปเดตบทบาท...</p>{/if}
		{#if !canReadRoles}
			<PageState
				variant="permission"
				title="ไม่มีสิทธิ์ดูรายการบทบาท"
				description="บัญชีนี้เข้า module บทบาทได้ แต่ยังไม่มีสิทธิ์อ่านรายการบทบาททั้งหมด"
			/>
		{:else if loading && !rolesLoaded}
			<div role="status" aria-label="กำลังโหลดบทบาท"><PageSkeleton variant="cards" rows={6} /></div>
		{:else if rolesError && !rolesLoaded}
			<PageState
				variant="error"
				title="โหลดบทบาทไม่สำเร็จ"
				description={rolesError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadRoles}
			/>
		{:else if roles.length === 0}
			<PageState
				title="ยังไม่มีบทบาท"
				description="เริ่มต้นสร้างบทบาทแรกของคุณได้เมื่อมีสิทธิ์สร้างบทบาท"
				actionLabel={canCreateRoles ? 'สร้างบทบาทใหม่' : undefined}
				onaction={() => goto(resolve('staff/roles/new'))}
			/>
		{:else}
			<!-- Roles Grid -->
			<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
				{#each roles as role (role.id)}
					<Card
						class={`cursor-pointer transition-shadow hover:shadow-lg ${role.is_active ? '' : 'opacity-70'}`}
						onclick={() => goto(resolve(`staff/roles/${role.id}`))}
					>
						<CardHeader>
							<div class="flex items-start justify-between gap-3">
								<div class="flex-1 min-w-0">
									<div class="flex items-center gap-2">
										<CardTitle class="truncate">{role.name}</CardTitle>
										{#if role.is_system}
											<Badge variant="outline">ระบบ</Badge>
										{/if}
									</div>
									<CardDescription class="truncate">{role.code}</CardDescription>
								</div>
								<Badge class="{getLevelBadgeColor(role.level)} text-white">
									Lv {role.level}
								</Badge>
							</div>
						</CardHeader>
						<CardContent class="space-y-3">
							<!-- Description -->
							{#if role.description}
								<p class="text-sm text-muted-foreground line-clamp-2">
									{role.description}
								</p>
							{/if}

							<!-- Stats -->
							<div class="flex items-center gap-4 text-sm">
								<div class="flex items-center gap-1 text-muted-foreground">
									<Shield class="h-4 w-4" />
									<span>
										{role.permissions.includes('*') ? 'All' : role.permissions.length} สิทธิ์
									</span>
								</div>
								<Badge class={getUserTypeBadgeColor(role.user_type)} variant="secondary">
									{role.user_type === 'staff'
										? 'บุคลากร'
										: role.user_type === 'student'
											? 'นักเรียน'
											: 'ผู้ปกครอง'}
								</Badge>
							</div>

							<!-- Status -->
							<div class="flex items-center justify-between pt-2 border-t">
								<Badge variant={role.is_active ? 'default' : 'secondary'}>
									{role.is_active ? 'กำลังใช้งาน' : 'ปิดใช้งาน'}
								</Badge>
								<Button
									variant="ghost"
									size="sm"
									onclick={(e: MouseEvent) => {
										e.stopPropagation();
										goto(resolve(`staff/roles/${role.id}`));
									}}
									class="gap-1"
								>
									{#if canUpdateRoles}
										<Edit class="h-3 w-3" />
										แก้ไข
									{:else}
										<Eye class="h-3 w-3" />
										ดูรายละเอียด
									{/if}
								</Button>
							</div>
						</CardContent>
					</Card>
				{/each}
			</div>
		{/if}
	</section>
</PageShell>

<style>
	:global(body) {
		font-family: 'Kanit', sans-serif;
	}
</style>
