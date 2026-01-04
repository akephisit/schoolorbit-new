<script lang="ts">
	import { onMount } from 'svelte';
	import { userRoleAPI, roleAPI, type UserRole, type Role } from '$lib/api/roles';
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
	import { Shield, Plus, Trash2, Star } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	interface Props {
		userId: string;
	}

	let { userId }: Props = $props();

	let userRoles = $state<UserRole[]>([]);
	let availableRoles = $state<Role[]>([]);
	let permissions = $state<string[]>([]);
	let loading = $state(true);
	let showAssignDialog = $state(false);
	let selectedRoleId = $state<string | undefined>(undefined);
	let isPrimary = $state(false);
	let assigning = $state(false);

	onMount(async () => {
		await Promise.all([loadUserRoles(), loadAvailableRoles(), loadPermissions()]);
		loading = false;
	});

	async function loadUserRoles() {
		try {
			const response = await userRoleAPI.getUserRoles(userId);
			if (response.success && response.data) {
				userRoles = response.data;
			}
		} catch (error) {
			console.error('Failed to load user roles:', error);
			toast.error('ไม่สามารถโหลดข้อมูล roles ได้');
		}
	}

	async function loadAvailableRoles() {
		try {
			const response = await roleAPI.listRoles();
			if (response.success && response.data) {
				availableRoles = response.data.filter((r: Role) => r.is_active);
			}
		} catch (error) {
			console.error('Failed to load roles:', error);
		}
	}

	async function loadPermissions() {
		try {
			const response = await userRoleAPI.getUserPermissions(userId);
			if (response.success && response.data) {
				permissions = response.data;
			}
		} catch (error) {
			console.error('Failed to load permissions:', error);
		}
	}

	async function handleAssignRole() {
		if (!selectedRoleId) {
			toast.error('กรุณาเลือกบทบาท');
			return;
		}

		assigning = true;
		try {
			const response = await userRoleAPI.assignRole(userId, {
				role_id: selectedRoleId,
				is_primary: isPrimary,
				started_at: new Date().toISOString().split('T')[0]
			});

			if (response.success) {
				toast.success('มอบหมายบทบาทสำเร็จ');
				showAssignDialog = false;
				selectedRoleId = undefined;
				isPrimary = false;
				await Promise.all([loadUserRoles(), loadPermissions()]);
			} else {
				toast.error(response.error || 'ไม่สามารถมอบหมายบทบาทได้');
			}
		} catch (error) {
			console.error('Failed to assign role:', error);
			toast.error('เกิดข้อผิดพลาด');
		} finally {
			assigning = false;
		}
	}

	async function handleRemoveRole(roleId: string, roleName: string) {
		if (!confirm(`คุณแน่ใจหรือไม่ที่จะเพิกถอนบทบาท "${roleName}"?`)) {
			return;
		}

		try {
			const response = await userRoleAPI.removeRole(userId, roleId);
			if (response.success) {
				toast.success('เพิกถอนบทบาทสำเร็จ');
				await Promise.all([loadUserRoles(), loadPermissions()]);
			} else {
				toast.error(response.error || 'ไม่สามารถเพิกถอนบทบาทได้');
			}
		} catch (error) {
			console.error('Failed to remove role:', error);
			toast.error('เกิดข้อผิดพลาด');
		}
	}

	function getRoleById(roleId: string): Role | undefined {
		return availableRoles.find((r) => r.id === roleId);
	}

	function getUnassignedRoles(): Role[] {
		const assignedRoleIds = new Set(userRoles.map((ur) => ur.role_id));
		return availableRoles.filter((r) => !assignedRoleIds.has(r.id));
	}
</script>

<div class="space-y-6">
	<Card>
		<CardHeader>
			<div class="flex items-center justify-between">
				<div>
					<CardTitle>บทบาทที่ได้รับ</CardTitle>
					<CardDescription>บทบาทและสิทธิ์การเข้าถึงของผู้ใช้งาน</CardDescription>
				</div>
				<Button onclick={() => (showAssignDialog = true)} size="sm" class="gap-2">
					<Plus class="h-4 w-4" />
					เพิ่มบทบาท
				</Button>
			</div>
		</CardHeader>
		<CardContent>
			{#if loading}
				<div class="space-y-2">
					{#each Array(2) as _, idx (idx)}
						<div class="h-16 bg-gray-100 rounded animate-pulse"></div>
					{/each}
				</div>
			{:else if userRoles.length === 0}
				<div class="text-center py-8">
					<Shield class="h-12 w-12 text-gray-400 mx-auto mb-2" />
					<p class="text-gray-600">ยังไม่มีบทบาทที่ได้รับ</p>
					<Button
						onclick={() => (showAssignDialog = true)}
						variant="outline"
						size="sm"
						class="mt-4 gap-2"
					>
						<Plus class="h-4 w-4" />
						เพิ่มบทบาท
					</Button>
				</div>
			{:else}
				<div class="space-y-2">
					{#each userRoles as userRole (userRole.id)}
						{@const role = getRoleById(userRole.role_id)}
						{#if role}
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
								<Button
									variant="ghost"
									size="sm"
									onclick={() => handleRemoveRole(role.id, role.name)}
									class="gap-1 text-red-600 hover:text-red-700 hover:bg-red-50"
								>
									<Trash2 class="h-3 w-3" />
									เพิกถอน
								</Button>
							</div>
						{/if}
					{/each}
				</div>
			{/if}
		</CardContent>
	</Card>

	<Card>
		<CardHeader>
			<CardTitle>สิทธิ์ที่มีผล</CardTitle>
			<CardDescription>สิทธิ์รวมจากบทบาททั้งหมด</CardDescription>
		</CardHeader>
		<CardContent>
			{#if loading}
				<div class="h-20 bg-gray-100 rounded animate-pulse"></div>
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
</div>

<Dialog bind:open={showAssignDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>เพิ่มบทบาท</DialogTitle>
			<DialogDescription>เลือกบทบาทที่ต้องการมอบหมายให้ผู้ใช้งาน</DialogDescription>
		</DialogHeader>

		<div class="space-y-4 py-4">
			<div class="space-y-2">
				<Label for="role">เลือกบทบาท</Label>
				<select
					id="role"
					bind:value={selectedRoleId}
					class="w-full px-3 py-2 border rounded-md bg-white"
				>
					<option value={undefined}>เลือกบทบาท...</option>
					{#each getUnassignedRoles() as role (role.id)}
						<option value={role.id}>{role.name} ({role.code})</option>
					{/each}
				</select>
			</div>

			<div class="flex items-center gap-2">
				<input type="checkbox" id="is_primary" bind:checked={isPrimary} class="rounded" />
				<Label for="is_primary">ตั้งเป็นบทบาทหลัก</Label>
			</div>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => (showAssignDialog = false)}>ยกเลิก</Button>
			<Button onclick={handleAssignRole} disabled={assigning || !selectedRoleId}>
				{assigning ? 'กำลังมอบหมาย...' : 'มอบหมาย'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
