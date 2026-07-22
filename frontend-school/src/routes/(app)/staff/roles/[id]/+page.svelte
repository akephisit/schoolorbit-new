<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import type { PageProps } from './$types';
	import { resolve } from '$app/paths';
	import { SvelteSet } from 'svelte/reactivity';
	import { roleAPI, permissionAPI, type Role, type PermissionsByModule } from '$lib/api/roles';
	import {
		PERMISSIONS,
		permissionActionLabel,
		permissionScopeMeta,
		permissionScopeToneClass
	} from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Switch } from '$lib/components/ui/switch';
	import { Alert, AlertDescription, AlertTitle } from '$lib/components/ui/alert';
	import * as Select from '$lib/components/ui/select';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '$lib/components/ui/dialog';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Badge } from '$lib/components/ui/badge';
	import { AlertTriangle, Power, RotateCcw, Save, Shield } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let { params }: PageProps = $props();
	let roleId = $derived(params.id);
	let isNew = $derived(roleId === 'new');

	let loading = $state(true);
	let saving = $state(false);
	let deactivating = $state(false);
	let reactivating = $state(false);
	let showDeactivateDialog = $state(false);
	let rolePermissionListLoading = $state(true);
	let initialRoleIsActive = $state<boolean | null>(null);

	// Role data
	let role = $state<Partial<Role>>({
		code: '',
		name: '',
		name_en: '',
		description: '',
		user_type: 'staff', // Changed from category to user_type
		level: 10,
		permissions: [],
		is_active: true,
		is_system: false
	});

	// Permissions
	let permissionsByModule = $state<PermissionsByModule>({});
	let selectedPermissions = new SvelteSet<string>();

	const canReadRoles = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const canCreateRoles = $derived($can.has(PERMISSIONS.ROLES_CREATE_ALL));
	const canUpdateRoles = $derived($can.has(PERMISSIONS.ROLES_UPDATE_ALL));
	const canDeleteRoles = $derived($can.has(PERMISSIONS.ROLES_DELETE_ALL));
	const canReadPermissionCatalog = $derived($can.has(PERMISSIONS.SETTINGS_READ_ALL));
	const canUsePage = $derived(isNew ? canCreateRoles : canReadRoles);
	const canEditRole = $derived(isNew ? canCreateRoles : canUpdateRoles);

	onMount(async () => {
		if (!canUsePage) {
			loading = false;
			rolePermissionListLoading = false;
			return;
		}

		if (canReadPermissionCatalog) {
			await loadPermissions();
		} else {
			rolePermissionListLoading = false;
		}

		if (!isNew && canReadRoles) {
			await loadRole();
		}
		loading = false;
	});

	async function loadRole() {
		try {
			const response = await roleAPI.getRole(roleId);
			if (response.success && response.data) {
				role = response.data;
				initialRoleIsActive = response.data.is_active;
				selectedPermissions.clear();
				for (const p of role.permissions || []) selectedPermissions.add(p);
			} else {
				toast.error('ไม่สามารถโหลดข้อมูล role ได้');
				goto(resolve('/staff/roles'));
			}
		} catch (error) {
			console.error('Failed to load role:', error);
			toast.error('เกิดข้อผิดพลาดในการโหลดข้อมูล');
			goto(resolve('/staff/roles'));
		}
	}

	async function loadPermissions() {
		rolePermissionListLoading = true;
		try {
			const response = await permissionAPI.listPermissionsByModule();
			if (response.success && response.data) {
				permissionsByModule = response.data;
			}
		} catch (error) {
			console.error('Failed to load permissions:', error);
			toast.error('ไม่สามารถโหลดรายการสิทธิ์ได้');
		} finally {
			rolePermissionListLoading = false;
		}
	}

	function togglePermission(code: string) {
		if (!canEditRole) return;
		if (selectedPermissions.has(code)) {
			selectedPermissions.delete(code);
		} else {
			selectedPermissions.add(code);
		}
		// Note: Parent module state is automatically reflected via isModuleFullySelected and isModulePartiallySelected
	}

	function toggleModule(module: string) {
		if (!canEditRole) return;
		const modulePermissions = permissionsByModule[module] || [];
		const allSelected = modulePermissions.every((p) => selectedPermissions.has(p.code));

		if (allSelected) {
			modulePermissions.forEach((p) => selectedPermissions.delete(p.code));
		} else {
			modulePermissions.forEach((p) => selectedPermissions.add(p.code));
		}
	}

	function isModuleFullySelected(module: string): boolean {
		const modulePermissions = permissionsByModule[module] || [];
		return (
			modulePermissions.length > 0 &&
			modulePermissions.every((p) => selectedPermissions.has(p.code))
		);
	}

	function isModulePartiallySelected(module: string): boolean {
		const modulePermissions = permissionsByModule[module] || [];
		const selected = modulePermissions.filter((p) => selectedPermissions.has(p.code)).length;
		return selected > 0 && selected < modulePermissions.length;
	}

	async function handleSave() {
		if (!canEditRole) {
			toast.error('ไม่มีสิทธิ์บันทึกบทบาท');
			return;
		}

		if (!role.code || !role.name) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}

		saving = true;
		try {
			const commonData = {
				name: role.name!,
				name_en: role.name_en,
				description: role.description,
				user_type: role.user_type!,
				level: role.level,
				permissions: Array.from(selectedPermissions)
			};

			if (isNew) {
				const response = await roleAPI.createRole({
					code: role.code!,
					...commonData
				});
				if (response.success) {
					toast.success('สร้างบทบาทสำเร็จ');
					goto(resolve('/staff/roles'));
				} else {
					toast.error(response.error || 'ไม่สามารถสร้างบทบาทได้');
				}
			} else {
				const statusChanged = initialRoleIsActive !== role.is_active;
				if (statusChanged && role.is_active === false && !canDeleteRoles) {
					toast.error('ต้องมีสิทธิ์ปิดใช้งานบทบาทก่อนเปลี่ยนสถานะ');
					return;
				}
				const response = await roleAPI.updateRole(roleId, {
					...commonData,
					...(statusChanged ? { is_active: role.is_active } : {})
				});
				if (response.success) {
					toast.success('บันทึกข้อมูลสำเร็จ');
					goto(resolve('/staff/roles'));
				} else {
					toast.error(response.error || 'ไม่สามารถบันทึกข้อมูลได้');
				}
			}
		} catch (error) {
			console.error('Failed to save role:', error);
			toast.error('เกิดข้อผิดพลาดในการบันทึกข้อมูล');
		} finally {
			saving = false;
		}
	}

	async function handleDeactivate() {
		if (!canDeleteRoles) {
			toast.error('ไม่มีสิทธิ์ปิดใช้งานบทบาท');
			return;
		}
		if (role.is_system) {
			toast.error('ไม่สามารถปิดใช้งานบทบาทระบบได้');
			return;
		}

		deactivating = true;
		try {
			const response = await roleAPI.deleteRole(roleId);
			if (response.success) {
				toast.success('ปิดใช้งานบทบาทสำเร็จ');
				showDeactivateDialog = false;
				goto(resolve('/staff/roles'));
			} else {
				toast.error(response.error || 'ไม่สามารถปิดใช้งานบทบาทได้');
				showDeactivateDialog = false;
			}
		} catch (error) {
			console.error('Failed to deactivate role:', error);
			toast.error(error instanceof Error ? error.message : 'เกิดข้อผิดพลาดในการปิดใช้งาน');
			showDeactivateDialog = false;
		} finally {
			deactivating = false;
		}
	}

	async function handleReactivate() {
		if (!canUpdateRoles) {
			toast.error('ไม่มีสิทธิ์เปิดใช้งานบทบาท');
			return;
		}

		reactivating = true;
		try {
			const response = await roleAPI.updateRole(roleId, { is_active: true });
			if (response.success) {
				toast.success('เปิดใช้งานบทบาทสำเร็จ');
				goto(resolve('/staff/roles'));
			} else {
				toast.error(response.error || 'ไม่สามารถเปิดใช้งานบทบาทได้');
			}
		} catch (error) {
			console.error('Failed to reactivate role:', error);
			toast.error(error instanceof Error ? error.message : 'เกิดข้อผิดพลาดในการเปิดใช้งาน');
		} finally {
			reactivating = false;
		}
	}

	function userTypeLabel(userType: string | undefined) {
		if (userType === 'student') return 'นักเรียน (Student)';
		if (userType === 'parent') return 'ผู้ปกครอง (Parent)';
		return 'บุคลากร (Staff)';
	}
</script>

<svelte:head>
	<title
		>{isNew ? 'สร้างบทบาทใหม่' : canUpdateRoles ? 'แก้ไขบทบาท' : 'รายละเอียดบทบาท'} - SchoolOrbit</title
	>
</svelte:head>

<PageShell
	title={isNew ? 'สร้างบทบาทใหม่' : canUpdateRoles ? 'แก้ไขบทบาท' : 'รายละเอียดบทบาท'}
	description={canEditRole ? 'กำหนดข้อมูลและสิทธิ์การเข้าถึง' : 'ดูข้อมูลและสิทธิ์ของบทบาท'}
	backHref="/staff/roles"
>
	{#snippet actions()}
		<div class="flex gap-2">
			{#if !loading && !isNew && initialRoleIsActive === true && canDeleteRoles && !role.is_system}
				<Button variant="destructive" onclick={() => (showDeactivateDialog = true)} class="gap-2">
					<Power class="h-4 w-4" />
					ปิดใช้งาน
				</Button>
			{:else if !loading && !isNew && initialRoleIsActive === false && canUpdateRoles}
				<Button variant="outline" onclick={handleReactivate} disabled={reactivating} class="gap-2">
					<RotateCcw class="h-4 w-4" />
					{reactivating ? 'กำลังเปิดใช้งาน...' : 'เปิดใช้งาน'}
				</Button>
			{/if}
			{#if canEditRole}
				<Button onclick={handleSave} disabled={saving} class="gap-2">
					<Save class="h-4 w-4" />
					{saving ? 'กำลังบันทึก...' : 'บันทึก'}
				</Button>
			{/if}
		</div>
	{/snippet}

	{#if !canUsePage}
		<PageState
			variant="permission"
			title={isNew ? 'ไม่มีสิทธิ์สร้างบทบาท' : 'ไม่มีสิทธิ์ดูบทบาท'}
			description="บัญชีนี้เข้า module บทบาทได้ แต่ยังไม่มีสิทธิ์สำหรับการทำงานในหน้านี้"
		/>
	{:else if loading}
		<PageSkeleton variant="form" rows={4} />
	{:else}
		<div class="space-y-6">
			<Card>
				<CardHeader>
					<div class="flex items-center gap-2">
						<CardTitle>ข้อมูลพื้นฐาน</CardTitle>
						{#if role.is_system}
							<Badge variant="outline">บทบาทระบบ</Badge>
						{/if}
						{#if !role.is_active}
							<Badge variant="secondary">ปิดใช้งาน</Badge>
						{/if}
					</div>
					<CardDescription>ข้อมูลทั่วไปของบทบาท</CardDescription>
				</CardHeader>
				<CardContent class="space-y-4">
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="code">รหัสบทบาท *</Label>
							<Input
								id="code"
								bind:value={role.code}
								placeholder="TEACHER"
								disabled={!isNew || !canEditRole}
								required
							/>
						</div>
						<div class="space-y-2">
							<Label for="level">ระดับ</Label>
							<Input
								id="level"
								type="number"
								bind:value={role.level}
								placeholder="10"
								disabled={!canEditRole}
							/>
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="name">ชื่อบทบาท (ไทย) *</Label>
							<Input
								id="name"
								bind:value={role.name}
								placeholder="ครูผู้สอน"
								disabled={!canEditRole}
								required
							/>
						</div>
						<div class="space-y-2">
							<Label for="name_en">ชื่อบทบาท (อังกฤษ)</Label>
							<Input
								id="name_en"
								bind:value={role.name_en}
								placeholder="Teacher"
								disabled={!canEditRole}
							/>
						</div>
					</div>

					<div class="space-y-2">
						<Label for="description">คำอธิบาย</Label>
						<Textarea
							id="description"
							bind:value={role.description}
							placeholder="อธิบายบทบาทและหน้าที่"
							rows={3}
							disabled={!canEditRole}
						/>
					</div>

					<div class="space-y-2">
						<Label for="user_type">ประเภทผู้ใช้ *</Label>
						<Select.Root type="single" bind:value={role.user_type} disabled={!canEditRole}>
							<Select.Trigger id="user_type" class="w-full">
								{userTypeLabel(role.user_type)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="staff">บุคลากร (Staff)</Select.Item>
								<Select.Item value="student">นักเรียน (Student)</Select.Item>
								<Select.Item value="parent">ผู้ปกครอง (Parent)</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>

					<div class="flex items-center gap-2">
						<Switch
							id="is_active"
							bind:checked={role.is_active}
							disabled={isNew ||
								!canEditRole ||
								role.is_system ||
								(initialRoleIsActive === true && !canDeleteRoles)}
						/>
						<Label for="is_active">เปิดใช้งาน</Label>
						{#if role.is_system}
							<span class="text-xs text-muted-foreground">บทบาทระบบไม่สามารถปิดใช้งานได้</span>
						{:else if role.is_active && !canDeleteRoles && !isNew}
							<span class="text-xs text-muted-foreground">ต้องมีสิทธิ์ปิดใช้งานบทบาท</span>
						{/if}
					</div>
				</CardContent>
			</Card>

			<Card>
				<CardHeader>
					<div class="flex items-center justify-between">
						<div>
							<CardTitle>สิทธิ์การเข้าถึง</CardTitle>
							<CardDescription>
								เลือกสิทธิ์ที่บทบาทนี้สามารถเข้าถึงได้ ({selectedPermissions.size} สิทธิ์)
							</CardDescription>
						</div>
						<Badge variant="secondary" class="gap-1">
							<Shield class="h-3 w-3" />
							{selectedPermissions.size} สิทธิ์
						</Badge>
					</div>
				</CardHeader>
				<CardContent>
					{#if !canReadPermissionCatalog}
						<Alert>
							<AlertTriangle class="h-4 w-4" />
							<AlertTitle>ไม่มีสิทธิ์ดูรายการ permission catalog</AlertTitle>
							<AlertDescription>
								ต้องมีสิทธิ์อ่านการตั้งค่าระบบก่อนจึงจะเลือกสิทธิ์ให้บทบาทได้
							</AlertDescription>
						</Alert>
					{:else if rolePermissionListLoading}
						<div class="py-8 text-center">
							<p class="text-muted-foreground">กำลังโหลดรายการสิทธิ์...</p>
						</div>
					{:else}
						<div class="space-y-4">
							{#each Object.entries(permissionsByModule) as [module, permissions] (module)}
								<div class="border rounded-lg p-4">
									<div class="flex items-center gap-2 mb-3">
										<Checkbox
											checked={isModuleFullySelected(module)}
											indeterminate={isModulePartiallySelected(module)}
											onCheckedChange={() => toggleModule(module)}
											disabled={!canEditRole}
										/>
										<button
											onclick={() => toggleModule(module)}
											disabled={!canEditRole}
											class="flex-1 text-left font-medium text-foreground hover:text-foreground/80"
										>
											{module}
											<span class="text-sm text-muted-foreground font-normal ml-2">
												({permissions.length} สิทธิ์)
											</span>
										</button>
									</div>

									<div class="grid grid-cols-2 gap-2 ml-6">
										{#each permissions as permission (permission.code)}
											{@const scopeMeta = permissionScopeMeta(permission.scope)}
											<label
												class="flex items-center gap-2 p-2 rounded hover:bg-gray-50 cursor-pointer"
											>
												<Checkbox
													checked={selectedPermissions.has(permission.code)}
													onCheckedChange={() => togglePermission(permission.code)}
													disabled={!canEditRole}
												/>
												<div class="flex-1 min-w-0">
													<div class="flex flex-wrap items-center gap-1.5">
														<p class="text-sm font-medium text-foreground truncate">
															{permission.name}
														</p>
														<Badge variant="outline" class="text-[11px]">
															{permissionActionLabel(permission.action)}
														</Badge>
														<Badge
															variant="outline"
															class={`text-[11px] ${permissionScopeToneClass(scopeMeta.tone)}`}
														>
															{scopeMeta.label}
														</Badge>
													</div>
													<p class="text-xs text-muted-foreground truncate">{permission.code}</p>
													<p class="text-xs text-muted-foreground line-clamp-2">
														{scopeMeta.description}
													</p>
												</div>
											</label>
										{/each}
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</CardContent>
			</Card>

			<div class="flex justify-end gap-2">
				<Button variant="outline" onclick={() => goto(resolve('/staff/roles'))}>ยกเลิก</Button>
				{#if canEditRole}
					<Button onclick={handleSave} disabled={saving} class="gap-2">
						<Save class="h-4 w-4" />
						{saving ? 'กำลังบันทึก...' : 'บันทึก'}
					</Button>
				{/if}
			</div>
		</div>
	{/if}
</PageShell>

<!-- Deactivation Confirmation Dialog -->
<Dialog bind:open={showDeactivateDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>ยืนยันการปิดใช้งานบทบาท</DialogTitle>
			<DialogDescription>
				ผู้ใช้ที่ได้รับบทบาท <strong>{role.name}</strong> จะสูญเสียสิทธิ์จากบทบาทนี้ทันที แต่ข้อมูลและการมอบหมายเดิมจะยังคงอยู่
				และสามารถเปิดใช้งานกลับเพื่อคืนสิทธิ์ได้ภายหลัง
			</DialogDescription>
		</DialogHeader>
		<DialogFooter>
			<Button
				variant="outline"
				onclick={() => (showDeactivateDialog = false)}
				disabled={deactivating}
			>
				ยกเลิก
			</Button>
			<Button
				variant="destructive"
				onclick={handleDeactivate}
				disabled={deactivating}
				class="gap-2"
			>
				<Power class="h-4 w-4" />
				{deactivating ? 'กำลังปิดใช้งาน...' : 'ปิดใช้งานบทบาท'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
