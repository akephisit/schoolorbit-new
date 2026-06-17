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
	import { AlertTriangle, ArrowLeft, Save, Trash2, Shield } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let { params }: PageProps = $props();
	let roleId = $derived(params.id);
	let isNew = $derived(roleId === 'new');

	let loading = $state(true);
	let saving = $state(false);
	let deleting = $state(false);
	let showDeleteDialog = $state(false);
	let rolePermissionListLoading = $state(true);

	// Role data
	let role = $state<Partial<Role>>({
		code: '',
		name: '',
		name_en: '',
		description: '',
		user_type: 'staff', // Changed from category to user_type
		level: 10,
		permissions: [],
		is_active: true
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
			const data = {
				code: role.code!,
				name: role.name!,
				name_en: role.name_en,
				description: role.description,
				user_type: role.user_type!, // Changed from category to user_type
				level: role.level,
				permissions: Array.from(selectedPermissions),
				is_active: role.is_active
			};

			if (isNew) {
				const response = await roleAPI.createRole(data);
				if (response.success) {
					toast.success('สร้างบทบาทสำเร็จ');
					goto(resolve('/staff/roles'));
				} else {
					toast.error(response.error || 'ไม่สามารถสร้างบทบาทได้');
				}
			} else {
				const response = await roleAPI.updateRole(roleId, data);
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

	async function handleDelete() {
		if (!canDeleteRoles) {
			toast.error('ไม่มีสิทธิ์ลบบทบาท');
			return;
		}

		deleting = true;
		try {
			const response = await roleAPI.deleteRole(roleId);
			if (response.success) {
				toast.success('ลบบทบาทสำเร็จ');
				showDeleteDialog = false;
				goto(resolve('/staff/roles'));
			} else {
				toast.error(response.error || 'ไม่สามารถลบบทบาทได้');
				showDeleteDialog = false;
			}
		} catch (error) {
			console.error('Failed to delete role:', error);
			toast.error('เกิดข้อผิดพลาดในการลบข้อมูล');
			showDeleteDialog = false;
		} finally {
			deleting = false;
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

<div class="space-y-6">
	<div class="flex items-center gap-4">
		<Button variant="ghost" size="icon" onclick={() => goto(resolve('/staff/roles'))}>
			<ArrowLeft class="h-5 w-5" />
		</Button>
		<div class="flex-1">
			<h1 class="text-3xl font-bold text-foreground">
				{isNew ? 'สร้างบทบาทใหม่' : canUpdateRoles ? 'แก้ไขบทบาท' : 'รายละเอียดบทบาท'}
			</h1>
			<p class="text-muted-foreground mt-1">
				{canEditRole ? 'กำหนดข้อมูลและสิทธิ์การเข้าถึง' : 'ดูข้อมูลและสิทธิ์ของบทบาท'}
			</p>
		</div>
		<div class="flex gap-2">
			{#if !isNew && canDeleteRoles}
				<Button variant="destructive" onclick={() => (showDeleteDialog = true)} class="gap-2">
					<Trash2 class="h-4 w-4" />
					ลบ
				</Button>
			{/if}
			{#if canEditRole}
				<Button onclick={handleSave} disabled={saving} class="gap-2">
					<Save class="h-4 w-4" />
					{saving ? 'กำลังบันทึก...' : 'บันทึก'}
				</Button>
			{/if}
		</div>
	</div>

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
					<CardTitle>ข้อมูลพื้นฐาน</CardTitle>
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
						<Switch id="is_active" bind:checked={role.is_active} disabled={!canEditRole} />
						<Label for="is_active">เปิดใช้งาน</Label>
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
</div>

<!-- Delete Confirmation Dialog -->
<Dialog bind:open={showDeleteDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>ยืนยันการลบบทบาท</DialogTitle>
			<DialogDescription>
				คุณแน่ใจหรือไม่ว่าต้องการลบบทบาท <strong>{role.name}</strong>?
				การกระทำนี้ไม่สามารถย้อนกลับได้
			</DialogDescription>
		</DialogHeader>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showDeleteDialog = false)} disabled={deleting}>
				ยกเลิก
			</Button>
			<Button variant="destructive" onclick={handleDelete} disabled={deleting} class="gap-2">
				<Trash2 class="h-4 w-4" />
				{deleting ? 'กำลังลบ...' : 'ลบบทบาท'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>
