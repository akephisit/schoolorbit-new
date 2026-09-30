<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';
	import { requireApiData } from '$lib/api/client';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { userRoleAPI, roleAPI, type UserRoleAssignment, type Role } from '$lib/api/roles';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Label } from '$lib/components/ui/label';
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { Shield, Plus, Trash2, Star } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	interface Props {
		userId: string;
		rolesRead: Promise<RouteLoadResult<UserRoleAssignment[]>>;
		permissionsRead: Promise<RouteLoadResult<string[]>>;
	}

	let { userId, rolesRead, permissionsRead }: Props = $props();

	let userRoles = $state<UserRoleAssignment[]>([]);
	let availableRoles = $state<Role[]>([]);
	let permissions = $state<string[]>([]);
	let loading = $state(true);
	let rolesLoaded = $state(false),
		permissionsLoaded = $state(false),
		effectivePermissionsLoading = $state(true);
	let rolesError = $state(''),
		permissionsError = $state(''),
		catalogError = $state(''),
		catalogLoading = $state(false);
	const rolesRequest = new LatestRequest(),
		permissionsRequest = new LatestRequest(),
		catalogRequest = new LatestRequest();
	const canRead = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const canAssign = $derived($can.has(PERMISSIONS.ROLES_ASSIGN_ALL));
	const canRemove = $derived($can.has(PERMISSIONS.ROLES_REMOVE_ALL));
	let activeId = '',
		disposed = false,
		ownerEpoch = 0,
		dialogEpoch = 0;
	let showAssignDialog = $state(false);
	let selectedRoleId = $state<string | undefined>(undefined);
	let isPrimary = $state(false);
	let assigning = $state(false);

	$effect.pre(() => {
		const id = userId;
		untrack(() => {
			if (activeId !== id) {
				activeId = id;
				ownerEpoch++;
				userRoles = [];
				permissions = [];
				rolesLoaded = false;
				permissionsLoaded = false;
				showAssignDialog = false;
				assigning = false;
			}
		});
	});
	$effect.pre(() => {
		const read = rolesRead;
		untrack(() => {
			const ticket = rolesRequest.begin();
			loading = true;
			rolesError = '';
			void read.then((result) => applyRoles(result, ticket.revision));
		});
		return () => rolesRequest.abort();
	});
	$effect.pre(() => {
		const read = permissionsRead;
		untrack(() => {
			const ticket = permissionsRequest.begin();
			effectivePermissionsLoading = true;
			permissionsError = '';
			void read.then((result) => applyPermissions(result, ticket.revision));
		});
		return () => permissionsRequest.abort();
	});
	$effect.pre(() => {
		const open = showAssignDialog,
			id = userId,
			allowed = canRead && canAssign;
		untrack(() => {
			dialogEpoch++;
			selectedRoleId = undefined;
			isPrimary = false;
			availableRoles = [];
			catalogError = '';
			if (open && allowed && id) void loadAvailableRoles();
			else {
				catalogRequest.abort();
				catalogLoading = false;
			}
		});
		return () => catalogRequest.abort();
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		rolesRequest.abort();
		permissionsRequest.abort();
		catalogRequest.abort();
	});
	function applyRoles(result: RouteLoadResult<UserRoleAssignment[]>, revision: number) {
		if (!rolesRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			userRoles = result.data;
			rolesLoaded = true;
		} else rolesError = result.error;
	}
	function applyPermissions(result: RouteLoadResult<string[]>, revision: number) {
		if (!permissionsRequest.isCurrent(revision)) return;
		effectivePermissionsLoading = false;
		if (result.ok) {
			permissions = result.data;
			permissionsLoaded = true;
		} else permissionsError = result.error;
	}
	async function loadUserRoles() {
		if (!canRead) return;
		const ticket = rolesRequest.begin();
		loading = true;
		rolesError = '';
		applyRoles(
			await captureRouteLoad(
				userRoleAPI
					.getUserRoles(userId, { signal: ticket.signal })
					.then((reply) => requireApiData(reply, 'โหลดบทบาทที่ได้รับไม่สำเร็จ')),
				'โหลดบทบาทที่ได้รับไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	async function loadPermissions() {
		if (!canRead) return;
		const ticket = permissionsRequest.begin();
		effectivePermissionsLoading = true;
		permissionsError = '';
		applyPermissions(
			await captureRouteLoad(
				userRoleAPI
					.getUserPermissions(userId, { signal: ticket.signal })
					.then((reply) => requireApiData(reply, 'โหลดสิทธิ์ที่มีผลไม่สำเร็จ')),
				'โหลดสิทธิ์ที่มีผลไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	async function loadAvailableRoles() {
		if (!showAssignDialog || !canRead || !canAssign) return;
		const ticket = catalogRequest.begin();
		catalogLoading = true;
		catalogError = '';
		const result = await captureRouteLoad(
			roleAPI
				.listRoles({}, { signal: ticket.signal })
				.then((reply) => requireApiData(reply, 'โหลดตัวเลือกบทบาทไม่สำเร็จ')),
			'โหลดตัวเลือกบทบาทไม่สำเร็จ'
		);
		if (!catalogRequest.isCurrent(ticket.revision)) return;
		catalogLoading = false;
		if (result.ok) availableRoles = result.data.filter((role) => role.is_active);
		else catalogError = result.error;
	}
	async function handleAssignRole() {
		if (!canAssign || !selectedRoleId || assigning || !showAssignDialog) return;
		const target = userId,
			epoch = ownerEpoch,
			draft = dialogEpoch;
		const current = () => !disposed && epoch === ownerEpoch && target === userId;
		rolesRequest.abort();
		permissionsRequest.abort();
		loading = false;
		effectivePermissionsLoading = false;
		assigning = true;
		try {
			const reply = await userRoleAPI.assignRole(target, {
				role_id: selectedRoleId,
				is_primary: isPrimary,
				started_at: new Date().toISOString().split('T')[0]
			});
			if (!current()) return;
			if (!reply.success) throw new Error(reply.error || 'ไม่สามารถมอบหมายบทบาทได้');
			if (draft === dialogEpoch) {
				toast.success('มอบหมายบทบาทสำเร็จ');
				showAssignDialog = false;
			}
			await Promise.all([loadUserRoles(), loadPermissions()]);
		} catch (e) {
			if (current() && draft === dialogEpoch)
				toast.error(e instanceof Error ? e.message : 'ไม่สามารถมอบหมายบทบาทได้');
		} finally {
			if (current()) assigning = false;
		}
	}
	async function handleRemoveRole(roleId: string, roleName: string) {
		if (!canRemove || assigning || !confirm(`คุณแน่ใจหรือไม่ที่จะเพิกถอนบทบาท "${roleName}"?`))
			return;
		const target = userId,
			epoch = ownerEpoch;
		const current = () => !disposed && epoch === ownerEpoch && target === userId;
		rolesRequest.abort();
		permissionsRequest.abort();
		loading = false;
		effectivePermissionsLoading = false;
		assigning = true;
		try {
			const reply = await userRoleAPI.removeRole(target, roleId);
			if (!current()) return;
			if (!reply.success) throw new Error(reply.error || 'ไม่สามารถเพิกถอนบทบาทได้');
			toast.success('เพิกถอนบทบาทสำเร็จ');
			await Promise.all([loadUserRoles(), loadPermissions()]);
		} catch (e) {
			if (current()) toast.error(e instanceof Error ? e.message : 'ไม่สามารถเพิกถอนบทบาทได้');
		} finally {
			if (current()) assigning = false;
		}
	}

	function getUnassignedRoles(): Role[] {
		const assignedRoleIds = new Set(userRoles.map((ur) => ur.role_id));
		return availableRoles.filter((r) => !assignedRoleIds.has(r.id));
	}
</script>

<div class="space-y-6">
	<section data-testid="user-role-assignments" aria-busy={loading}>
		<Card>
			<CardHeader>
				<div class="flex items-center justify-between">
					<div>
						<CardTitle>บทบาทที่ได้รับ</CardTitle>
						<CardDescription>บทบาทและสิทธิ์การเข้าถึงของผู้ใช้งาน</CardDescription>
					</div>
					{#if canAssign}<Button
							onclick={() => (showAssignDialog = true)}
							disabled={assigning}
							size="sm"
							class="gap-2"
						>
							<Plus class="h-4 w-4" />
							เพิ่มบทบาท
						</Button>{/if}
				</div>
			</CardHeader>
			<CardContent>
				{#if rolesError && rolesLoaded && canRead}<PageState
						title="อัปเดตบทบาทไม่สำเร็จ"
						description={rolesError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadUserRoles}
					/>{/if}
				{#if loading && rolesLoaded && canRead}<p role="status">กำลังอัปเดตบทบาท...</p>{/if}
				{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์อ่านบทบาทที่ได้รับ" />
				{:else if loading && !rolesLoaded}<div role="status" aria-label="กำลังโหลดบทบาทที่ได้รับ">
						<PageSkeleton variant="cards" rows={2} />
					</div>
				{:else if rolesError && !rolesLoaded}<PageState
						variant="error"
						title="โหลดบทบาทที่ได้รับไม่สำเร็จ"
						description={rolesError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadUserRoles}
					/>
				{:else if userRoles.length === 0}
					<div class="text-center py-8">
						<Shield class="h-12 w-12 text-gray-400 mx-auto mb-2" />
						<p class="text-gray-600">ยังไม่มีบทบาทที่ได้รับ</p>
						{#if canAssign}<Button
								onclick={() => (showAssignDialog = true)}
								variant="outline"
								size="sm"
								class="mt-4 gap-2"
							>
								<Plus class="h-4 w-4" />
								เพิ่มบทบาท
							</Button>{/if}
					</div>
				{:else}
					<div class="space-y-2">
						{#each userRoles as userRole (userRole.id)}
							{@const role = userRole.role}
							<div class="flex items-center justify-between p-3 border rounded-lg hover:bg-gray-50">
								<div class="flex items-center gap-3 flex-1">
									{#if userRole.is_primary}
										<Star class="h-4 w-4 text-yellow-500 fill-yellow-500" />
									{:else}
										<Shield class="h-4 w-4 text-gray-400" />
									{/if}
									<div class="flex-1 min-w-0">
										<div class="flex items-center gap-2">
											<p class="font-medium text-gray-900">{role.name}</p>
											{#if userRole.is_primary}
												<Badge variant="secondary" class="text-xs">หลัก</Badge>
											{/if}
										</div>
										<p class="text-sm text-gray-500">{role.code}</p>
									</div>
									<div class="text-sm text-gray-600">
										{role.permissions.includes('*')
											? 'ทุกสิทธิ์'
											: `${role.permissions.length} สิทธิ์`}
									</div>
								</div>
								{#if canRemove}<Button
										variant="ghost"
										size="sm"
										onclick={() => handleRemoveRole(role.id, role.name)}
										disabled={assigning}
										class="gap-1 text-red-600 hover:text-red-700 hover:bg-red-50"
									>
										<Trash2 class="h-3 w-3" />
										เพิกถอน
									</Button>{/if}
							</div>
						{/each}
					</div>
				{/if}
			</CardContent>
		</Card>
	</section>
	<section data-testid="user-effective-permissions" aria-busy={effectivePermissionsLoading}>
		<Card>
			<CardHeader>
				<CardTitle>สิทธิ์ที่มีผล</CardTitle>
				<CardDescription>สิทธิ์รวมจากบทบาททั้งหมด</CardDescription>
			</CardHeader>
			<CardContent>
				{#if permissionsError && permissionsLoaded && canRead}<PageState
						title="อัปเดตสิทธิ์ไม่สำเร็จ"
						description={permissionsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadPermissions}
					/>{/if}
				{#if effectivePermissionsLoading && permissionsLoaded && canRead}<p role="status">
						กำลังอัปเดตสิทธิ์...
					</p>{/if}
				{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์อ่านสิทธิ์ที่มีผล" />
				{:else if effectivePermissionsLoading && !permissionsLoaded}<div
						role="status"
						aria-label="กำลังโหลดสิทธิ์ที่มีผล"
					>
						<PageSkeleton variant="cards" rows={2} />
					</div>
				{:else if permissionsError && !permissionsLoaded}<PageState
						variant="error"
						title="โหลดสิทธิ์ที่มีผลไม่สำเร็จ"
						description={permissionsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadPermissions}
					/>
				{:else if permissions.length === 0}
					<p class="text-center text-gray-600 py-4">ยังไม่มีสิทธิ์</p>
				{:else if permissions.includes('*')}
					<div class="text-center py-4">
						<Badge class="bg-purple-500 text-white">ทุกสิทธิ์</Badge>
						<p class="text-sm text-gray-600 mt-2">มีสิทธิ์เข้าถึงทุกอย่าง</p>
					</div>
				{:else}
					<div class="flex flex-wrap gap-2">
						{#each permissions as permission (permission)}
							<Badge variant="secondary">{permission}</Badge>
						{/each}
					</div>
				{/if}
			</CardContent>
		</Card>
	</section>
</div>

{#if canAssign}<Dialog bind:open={showAssignDialog}>
		<DialogContent>
			<DialogHeader>
				<DialogTitle>เพิ่มบทบาท</DialogTitle>
				<DialogDescription>เลือกบทบาทที่ต้องการมอบหมายให้ผู้ใช้งาน</DialogDescription>
			</DialogHeader>

			{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์อ่านตัวเลือกบทบาท" />
			{:else if catalogLoading}<div role="status" aria-label="กำลังโหลดตัวเลือกบทบาท">
					<PageSkeleton variant="form" rows={2} />
				</div>
			{:else if catalogError}<PageState
					variant="error"
					title="โหลดตัวเลือกบทบาทไม่สำเร็จ"
					description={catalogError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadAvailableRoles}
				/>
			{:else}
				<div class="space-y-4 py-4">
					<div class="space-y-2">
						<Label for="role">เลือกบทบาท</Label>
						<Select.Root type="single" bind:value={selectedRoleId}>
							<Select.Trigger id="role" class="w-full">
								{@const selectedRole = availableRoles.find((role) => role.id === selectedRoleId)}
								{selectedRole ? `${selectedRole.name} (${selectedRole.code})` : 'เลือกบทบาท...'}
							</Select.Trigger>
							<Select.Content>
								{#each getUnassignedRoles() as role (role.id)}
									<Select.Item value={role.id}>{role.name} ({role.code})</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>

					<div class="flex items-center gap-2">
						<input type="checkbox" id="is_primary" bind:checked={isPrimary} class="rounded" />
						<Label for="is_primary">ตั้งเป็นบทบาทหลัก</Label>
					</div>
				</div>
			{/if}
			<DialogFooter>
				<Button variant="outline" onclick={() => (showAssignDialog = false)}>ยกเลิก</Button>
				<Button
					onclick={handleAssignRole}
					disabled={assigning || !selectedRoleId || catalogLoading || !!catalogError || !canRead}
				>
					{assigning ? 'กำลังมอบหมาย...' : 'มอบหมาย'}
				</Button>
			</DialogFooter>
		</DialogContent>
	</Dialog>{/if}
