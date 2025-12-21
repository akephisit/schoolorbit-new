<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { roleAPI, permissionAPI, type Role, type PermissionsByModule } from '$lib/api/roles';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Switch } from '$lib/components/ui/switch';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Badge } from '$lib/components/ui/badge';
	import { ArrowLeft, Save, Trash2, Shield } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let roleId = $page.params.id;
	let isNew = roleId === 'new';

	let loading = true;
	let saving = false;
	let permissionsLoading = true;

	// Role data
	let role: Partial<Role> = {
		code: '',
		name: '',
		name_en: '',
		description: '',
		category: 'academic',
		level: 10,
		permissions: [],
		is_active: true
	};

	// Permissions
	let permissionsByModule: PermissionsByModule = {};
	let selectedPermissions: Set<string> = new Set();

	onMount(async () => {
		await loadPermissions();
		if (!isNew) {
			await loadRole();
		}
		loading = false;
	});

	async function loadRole() {
		try {
			const response = await roleAPI.getRole(roleId);
			if (response.success && response.data) {
				role = response.data;
				selectedPermissions = new Set(role.permissions || []);
			} else {
				toast.error('ไม่สามารถโหลดข้อมูล role ได้');
				goto('/admin/roles');
			}
		} catch (error) {
			console.error('Failed to load role:', error);
			toast.error('เกิดข้อผิดพลาดในการโหลดข้อมูล');
			goto('/admin/roles');
		}
	}

	async function loadPermissions() {
		permissionsLoading = true;
		try {
			const response = await permissionAPI.listPermissionsByModule();
			if (response.success && response.data) {
				permissionsByModule = response.data;
			}
		} catch (error) {
			console.error('Failed to load permissions:', error);
			toast.error('ไม่สามารถโหลดรายการสิทธิ์ได้');
		} finally {
			permissionsLoading = false;
		}
	}

	function togglePermission(code: string) {
		if (selectedPermissions.has(code)) {
			selectedPermissions.delete(code);
		} else {
			selectedPermissions.add(code);
		}
		selectedPermissions = selectedPermissions; // Trigger reactivity
	}

	function toggleModule(module: string) {
		const modulePermissions = permissionsByModule[module] || [];
		const allSelected = modulePermissions.every((p) => selectedPermissions.has(p.code));

		if (allSelected) {
			// Unselect all
			modulePermissions.forEach((p) => selectedPermissions.delete(p.code));
		} else {
			// Select all
			modulePermissions.forEach((p) => selectedPermissions.add(p.code));
		}
		selectedPermissions = selectedPermissions;
	}

	function isModuleFullySelected(module: string): boolean {
		const modulePermissions = permissionsByModule[module] || [];
		return modulePermissions.length > 0 && modulePermissions.every((p) => selectedPermissions.has(p.code));
	}

	function isModulePartiallySelected(module: string): boolean {
		const modulePermissions = permissionsByModule[module] || [];
		const selected = modulePermissions.filter((p) => selectedPermissions.has(p.code)).length;
		return selected > 0 && selected < modulePermissions.length;
	}

	async function handleSave() {
		// Validation
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
				category: role.category,
				level: role.level,
				permissions: Array.from(selectedPermissions),
				is_active: role.is_active
			};

			if (isNew) {
				const response = await roleAPI.createRole(data);
				if (response.success) {
					toast.success('สร้างบทบาทสำเร็จ');
					goto('/admin/roles');
				} else {
					toast.error(response.error || 'ไม่สามารถสร้างบทบาทได้');
				}
			} else {
				const response = await roleAPI.updateRole(roleId, data);
				if (response.success) {
					toast.success('บันทึกข้อมูลสำเร็จ');
					goto('/admin/roles');
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
		if (!confirm('คุณแน่ใจหรือไม่ว่าต้องการลบบทบาทนี้?')) {
			return;
		}

		try {
			const response = await roleAPI.deleteRole(roleId);
			if (response.success) {
				toast.success('ลบบทบาทสำเร็จ');
				goto('/admin/roles');
			} else {
				toast.error(response.error || 'ไม่สามารถลบบทบาทได้');
			}
		} catch (error) {
			console.error('Failed to delete role:', error);
			toast.error('เกิดข้อผิดพลาดในการลบข้อมูล');
		}
	}
</script>

<svelte:head>
	<title>{isNew ? 'สร้างบทบาทใหม่' : 'แก้ไขบทบาท'} - SchoolOrbit</title>
</svelte:head>

<div class="container mx-auto py-6 px-4 max-w-4xl">
	<!-- Header -->
	<div class="flex items-center gap-4 mb-6">
		<Button variant="ghost" size="icon" on:click={() => goto('/admin/roles')}>
			<ArrowLeft class="h-5 w-5" />
		</Button>
		<div class="flex-1">
			<h1 class="text-3xl font-bold text-gray-900">
				{isNew ? 'สร้างบทบาทใหม่' : 'แก้ไขบทบาท'}
			</h1>
			<p class="text-gray-600 mt-1">กำหนดข้อมูลและสิทธิ์การเข้าถึง</p>
		</div>
		<div class="flex gap-2">
			{#if !isNew}
				<Button variant="destructive" on:click={handleDelete} class="gap-2">
					<Trash2 class="h-4 w-4" />
					ลบ
				</Button>
			{/if}
			<Button on:click={handleSave} disabled={saving} class="gap-2">
				<Save class="h-4 w-4" />
				{saving ? 'กำลังบันทึก...' : 'บันทึก'}
			</Button>
		</div>
	</div>

	{#if loading}
		<div class="space-y-4">
			<Card>
				<CardContent class="py-8">
					<div class="animate-pulse space-y-4">
						<div class="h-4 bg-gray-200 rounded w-3/4"></div>
						<div class="h-4 bg-gray-200 rounded w-1/2"></div>
					</div>
				</CardContent>
			</Card>
		</div>
	{:else}
		<div class="space-y-6">
			<!-- Basic Information -->
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
								disabled={!isNew}
								required
							/>
						</div>
						<div class="space-y-2">
							<Label for="level">ระดับ</Label>
							<Input id="level" type="number" bind:value={role.level} placeholder="10" />
						</div>
					</div>

					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="name">ชื่อบทบาท (ไทย) *</Label>
							<Input id="name" bind:value={role.name} placeholder="ครูผู้สอน" required />
						</div>
						<div class="space-y-2">
							<Label for="name_en">ชื่อบทบาท (อังกฤษ)</Label>
							<Input id="name_en" bind:value={role.name_en} placeholder="Teacher" />
						</div>
					</div>

					<div class="space-y-2">
						<Label for="description">คำอธิบาย</Label>
						<Textarea
							id="description"
							bind:value={role.description}
							placeholder="อธิบายบทบาทและหน้าที่"
							rows={3}
						/>
					</div>

					<div class="space-y-2">
						<Label for="category">ประเภท</Label>
						<select
							id="category"
							bind:value={role.category}
							class="w-full px-3 py-2 border rounded-md"
						>
							<option value="administrative">บริหาร (Administrative)</option>
							<option value="academic">วิชาการ (Academic)</option>
							<option value="support">สนับสนุน (Support)</option>
						</select>
					</div>

					<div class="flex items-center gap-2">
						<Switch id="is_active" bind:checked={role.is_active} />
						<Label for="is_active">เปิดใช้งาน</Label>
					</div>
				</CardContent>
			</Card>

			<!-- Permissions -->
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
					{#if permissionsLoading}
						<div class="py-8 text-center">
							<p class="text-gray-500">กำลังโหลดรายการสิทธิ์...</p>
						</div>
					{:else}
						<div class="space-y-4">
							{#each Object.entries(permissionsByModule) as [module, permissions]}
								<div class="border rounded-lg p-4">
									<!-- Module Header -->
									<div class="flex items-center gap-2 mb-3">
										<Checkbox
											checked={isModuleFullySelected(module)}
											indeterminate={isModulePartiallySelected(module)}
											on:click={() => toggleModule(module)}
										/>
										<button
											on:click={() => toggleModule(module)}
											class="flex-1 text-left font-medium text-gray-900 hover:text-gray-700"
										>
											{module}
											<span class="text-sm text-gray-500 font-normal ml-2">
												({permissions.length} สิทธิ์)
											</span>
										</button>
									</div>

									<!-- Permissions -->
									<div class="grid grid-cols-2 gap-2 ml-6">
										{#each permissions as permission}
											<label
												class="flex items-center gap-2 p-2 rounded hover:bg-gray-50 cursor-pointer"
											>
												<Checkbox
													checked={selectedPermissions.has(permission.code)}
													on:click={() => togglePermission(permission.code)}
												/>
												<div class="flex-1 min-w-0">
													<p class="text-sm font-medium text-gray-900 truncate">
														{permission.name}
													</p>
													<p class="text-xs text-gray-500 truncate">
														{permission.code}
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

			<!-- Actions -->
			<div class="flex justify-end gap-2">
				<Button variant="outline" on:click={() => goto('/admin/roles')}>ยกเลิก</Button>
				<Button on:click={handleSave} disabled={saving} class="gap-2">
					<Save class="h-4 w-4" />
					{saving ? 'กำลังบันทึก...' : 'บันทึก'}
				</Button>
			</div>
		</div>
	{/if}
</div>

<style>
	:global(body) {
		font-family: 'Kanit', sans-serif;
	}
</style>
