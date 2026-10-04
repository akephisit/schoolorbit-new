<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { requireApiData } from '#lib/api/client.js';
	import { goto } from '$app/navigation';
	import type { PageProps } from './$types';
	import { resolve } from '$app/paths';
	import { SvelteSet } from 'svelte/reactivity';
	import { roleAPI, permissionAPI, type Role, type PermissionsByModule } from '#lib/api/roles.js';
	import {
		PERMISSIONS,
		permissionActionLabel,
		permissionScopeMeta,
		permissionScopeToneClass
	} from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { Alert, AlertDescription, AlertTitle } from '#lib/components/ui/alert/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';

	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '#lib/components/ui/dialog/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { AlertTriangle, Power, RotateCcw, Save, Shield } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let { data }: PageProps = $props();
	const roleRead = $derived(data.role);
	const catalogRead = $derived(data.catalog);
	let roleId = $derived(data.roleId);
	let isNew = $derived(roleId === 'new');

	let loading = $state(true);
	let roleLoaded = $state(false);
	let roleError = $state('');
	let catalogLoaded = $state(false);
	let catalogError = $state('');
	const roleRequest = new LatestRequest();
	const catalogRequest = new LatestRequest();
	let roleEpoch = 0;
	let roleActive = false;
	let renderedRoleId = '';
	let saving = $state(false);
	let deactivating = $state(false);
	let reactivating = $state(false);
	let showDeactivateDialog = $state(false);
	let rolePermissionListLoading = $state(true);
	let initialRoleIsActive = $state<boolean | null>(null);

	// Role data
	const emptyRole = (): Partial<Role> => ({
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
	let role = $state<Partial<Role>>(emptyRole());

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

	$effect.pre(() => {
		const sourceRole = roleRead,
			sourceCatalog = catalogRead,
			owner = roleId;
		untrack(() => {
			roleActive = true;
			if (renderedRoleId !== owner) {
				renderedRoleId = owner;
				role = emptyRole();
				roleLoaded = false;
				catalogLoaded = false;
				permissionsByModule = {};
				selectedPermissions.clear();
				initialRoleIsActive = null;
				saving = false;
				deactivating = false;
				reactivating = false;
				showDeactivateDialog = false;
			}
			const roleTicket = roleRequest.begin(),
				catalogTicket = catalogRequest.begin();
			loading = true;
			roleError = '';
			rolePermissionListLoading = canReadPermissionCatalog;
			catalogError = '';
			void sourceRole.then((result) => applyRole(result, roleTicket.revision));
			void sourceCatalog.then((result) => applyCatalog(result, catalogTicket.revision));
		});
		return () => {
			roleActive = false;
			roleEpoch++;
			roleRequest.abort();
			catalogRequest.abort();
		};
	});
	function applyRole(result: Awaited<typeof data.role>, revision: number) {
		if (!roleRequest.isCurrent(revision)) return;
		loading = false;
		if (!result.ok) {
			roleError = result.error;
			return;
		}
		role = result.data ?? emptyRole();
		roleLoaded = true;
		initialRoleIsActive = result.data?.is_active ?? true;
		selectedPermissions.clear();
		for (const permission of result.data?.permissions ?? []) selectedPermissions.add(permission);
	}
	function applyCatalog(result: Awaited<typeof data.catalog>, revision: number) {
		if (!catalogRequest.isCurrent(revision)) return;
		rolePermissionListLoading = false;
		if (result.ok) {
			permissionsByModule = result.data ?? {};
			catalogLoaded = true;
		} else catalogError = result.error;
	}
	async function loadRole() {
		if (!canUsePage || isNew) return;
		const ticket = roleRequest.begin();
		loading = true;
		roleError = '';
		const result = await captureRouteLoad(
			roleAPI
				.getRole(roleId, { signal: ticket.signal })
				.then((response) => requireApiData(response, 'โหลดบทบาทไม่สำเร็จ')),
			'โหลดบทบาทไม่สำเร็จ'
		);
		applyRole(result, ticket.revision);
	}
	async function loadPermissions() {
		if (!canUsePage || !canReadPermissionCatalog) return;
		const ticket = catalogRequest.begin();
		rolePermissionListLoading = true;
		catalogError = '';
		const result = await captureRouteLoad(
			permissionAPI
				.listPermissionsByModule({ signal: ticket.signal })
				.then((response) => requireApiData(response, 'โหลดรายการสิทธิ์ไม่สำเร็จ')),
			'โหลดรายการสิทธิ์ไม่สำเร็จ'
		);
		applyCatalog(result, ticket.revision);
	}
	const mutationBusy = $derived(saving || deactivating || reactivating);

	function togglePermission(code: string) {
		if (!canEditRole || !roleLoaded || loading || mutationBusy) return;
		if (selectedPermissions.has(code)) {
			selectedPermissions.delete(code);
		} else {
			selectedPermissions.add(code);
		}
		// Note: Parent module state is automatically reflected via isModuleFullySelected and isModulePartiallySelected
	}

	function toggleModule(module: string) {
		if (!canEditRole || !roleLoaded || loading || mutationBusy) return;
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

		if (!roleActive || !roleLoaded || loading || mutationBusy) return;
		const sourceEpoch = roleEpoch;
		roleRequest.abort();
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
				if (sourceEpoch !== roleEpoch || !roleActive) return;
				if (response.success) {
					toast.success('สร้างบทบาทสำเร็จ');
					goto(resolve('staff/roles'));
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
				if (sourceEpoch !== roleEpoch || !roleActive) return;
				if (response.success) {
					toast.success('บันทึกข้อมูลสำเร็จ');
					goto(resolve('staff/roles'));
				} else {
					toast.error(response.error || 'ไม่สามารถบันทึกข้อมูลได้');
				}
			}
		} catch (error) {
			if (sourceEpoch !== roleEpoch || !roleActive) return;
			console.error('Failed to save role:', error);
			toast.error('เกิดข้อผิดพลาดในการบันทึกข้อมูล');
		} finally {
			if (sourceEpoch === roleEpoch && roleActive) saving = false;
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

		if (!roleActive || !roleLoaded || loading || mutationBusy) return;
		const sourceEpoch = roleEpoch;
		roleRequest.abort();
		deactivating = true;
		try {
			const response = await roleAPI.deleteRole(roleId);
			if (sourceEpoch !== roleEpoch || !roleActive) return;
			if (response.success) {
				toast.success('ปิดใช้งานบทบาทสำเร็จ');
				showDeactivateDialog = false;
				goto(resolve('staff/roles'));
			} else {
				toast.error(response.error || 'ไม่สามารถปิดใช้งานบทบาทได้');
				showDeactivateDialog = false;
			}
		} catch (error) {
			if (sourceEpoch !== roleEpoch || !roleActive) return;
			console.error('Failed to deactivate role:', error);
			toast.error(error instanceof Error ? error.message : 'เกิดข้อผิดพลาดในการปิดใช้งาน');
			showDeactivateDialog = false;
		} finally {
			if (sourceEpoch === roleEpoch && roleActive) deactivating = false;
		}
	}

	async function handleReactivate() {
		if (!canUpdateRoles) {
			toast.error('ไม่มีสิทธิ์เปิดใช้งานบทบาท');
			return;
		}

		if (!roleActive || !roleLoaded || loading || mutationBusy) return;
		const sourceEpoch = roleEpoch;
		roleRequest.abort();
		reactivating = true;
		try {
			const response = await roleAPI.updateRole(roleId, { is_active: true });
			if (sourceEpoch !== roleEpoch || !roleActive) return;
			if (response.success) {
				toast.success('เปิดใช้งานบทบาทสำเร็จ');
				goto(resolve('staff/roles'));
			} else {
				toast.error(response.error || 'ไม่สามารถเปิดใช้งานบทบาทได้');
			}
		} catch (error) {
			if (sourceEpoch !== roleEpoch || !roleActive) return;
			console.error('Failed to reactivate role:', error);
			toast.error(error instanceof Error ? error.message : 'เกิดข้อผิดพลาดในการเปิดใช้งาน');
		} finally {
			if (sourceEpoch === roleEpoch && roleActive) reactivating = false;
		}
	}

	function userTypeLabel(userType: string | undefined) {
		if (userType === 'student') return 'นักเรียน (Student)';
		if (userType === 'parent') return 'ผู้ปกครอง (Parent)';
		return 'บุคลากร (Staff)';
	}
</script>

<PageShell
	title={isNew ? 'สร้างบทบาทใหม่' : canUpdateRoles ? 'แก้ไขบทบาท' : 'รายละเอียดบทบาท'}
	description={canEditRole ? 'กำหนดข้อมูลและสิทธิ์การเข้าถึง' : 'ดูข้อมูลและสิทธิ์ของบทบาท'}
	backHref="/staff/roles"
	backPreload={false}
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
				<Button
					onclick={handleSave}
					disabled={mutationBusy || loading || !roleLoaded}
					class="gap-2"
				>
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
	{:else}
		<div class="space-y-6">
			<section data-testid="role-detail" aria-busy={loading}>
				{#if loading && !roleLoaded}<div role="status" aria-label="กำลังโหลดบทบาท">
						<PageSkeleton variant="form" rows={4} />
					</div>
				{:else if roleError && !roleLoaded}<PageState
						variant="error"
						title="โหลดบทบาทไม่สำเร็จ"
						description={roleError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadRole}
					/>
				{:else}
					{#if roleError}<PageState
							title="อัปเดตบทบาทไม่สำเร็จ"
							description={roleError}
							actionLabel="ลองอีกครั้ง"
							onaction={loadRole}
						/>{/if}
					{#if loading}<p role="status">กำลังอัปเดตบทบาท...</p>{/if}
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
										disabled={!canEditRole || loading || !roleLoaded || mutationBusy}
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
										disabled={!canEditRole || loading || !roleLoaded || mutationBusy}
										required
									/>
								</div>
								<div class="space-y-2">
									<Label for="name_en">ชื่อบทบาท (อังกฤษ)</Label>
									<Input
										id="name_en"
										bind:value={role.name_en}
										placeholder="Teacher"
										disabled={!canEditRole || loading || !roleLoaded || mutationBusy}
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
									disabled={!canEditRole || loading || !roleLoaded || mutationBusy}
								/>
							</div>

							<div class="space-y-2">
								<Label for="user_type">ประเภทผู้ใช้ *</Label>
								<Select.Root
									type="single"
									bind:value={role.user_type}
									disabled={!canEditRole || loading || !roleLoaded || mutationBusy}
								>
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
				{/if}
			</section>
			<section data-testid="role-permissions" aria-busy={rolePermissionListLoading}>
				<Card>
					<CardHeader>
						<div class="flex items-center justify-between">
							<div>
								<CardTitle>สิทธิ์การเข้าถึง</CardTitle>
								<CardDescription>
									เลือกสิทธิ์ที่บทบาทนี้สามารถเข้าถึงได้ ({roleLoaded
										? selectedPermissions.size
										: '—'} สิทธิ์)
								</CardDescription>
							</div>
							<Badge variant="secondary" class="gap-1">
								<Shield class="h-3 w-3" />
								{roleLoaded ? selectedPermissions.size : '—'} สิทธิ์
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
						{:else if rolePermissionListLoading && !catalogLoaded}
							<div role="status" aria-label="กำลังโหลดรายการสิทธิ์">
								<PageSkeleton variant="cards" rows={4} />
							</div>
						{:else if catalogError && !catalogLoaded}<PageState
								variant="error"
								title="โหลดรายการสิทธิ์ไม่สำเร็จ"
								description={catalogError}
								actionLabel="ลองอีกครั้ง"
								onaction={loadPermissions}
							/>
						{:else}
							{#if catalogError}<PageState
									title="อัปเดตรายการสิทธิ์ไม่สำเร็จ"
									description={catalogError}
									actionLabel="ลองอีกครั้ง"
									onaction={loadPermissions}
								/>{/if}
							{#if rolePermissionListLoading}<p role="status">กำลังอัปเดตรายการสิทธิ์...</p>{/if}
							<div class="space-y-4">
								{#each Object.entries(permissionsByModule) as [module, permissions] (module)}
									<div class="border rounded-lg p-4">
										<div class="flex items-center gap-2 mb-3">
											<Checkbox
												checked={isModuleFullySelected(module)}
												indeterminate={isModulePartiallySelected(module)}
												onCheckedChange={() => toggleModule(module)}
												disabled={!canEditRole || loading || !roleLoaded || mutationBusy}
											/>
											<button
												onclick={() => toggleModule(module)}
												disabled={!canEditRole || loading || !roleLoaded || mutationBusy}
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
														disabled={!canEditRole || loading || !roleLoaded || mutationBusy}
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
			</section>

			<div class="flex justify-end gap-2">
				<Button variant="outline" onclick={() => goto(resolve('staff/roles'))}>ยกเลิก</Button>
				{#if !isNew}<Button variant="outline" onclick={loadRole} disabled={loading || mutationBusy}
						>รีเฟรชบทบาท</Button
					>{/if}
				{#if canReadPermissionCatalog}<Button
						variant="outline"
						onclick={loadPermissions}
						disabled={rolePermissionListLoading || mutationBusy}>รีเฟรชรายการสิทธิ์</Button
					>{/if}
				{#if canEditRole}
					<Button
						onclick={handleSave}
						disabled={mutationBusy || loading || !roleLoaded}
						class="gap-2"
					>
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
