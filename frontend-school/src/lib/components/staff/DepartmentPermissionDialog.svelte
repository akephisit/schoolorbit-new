<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { permissionAPI, type PermissionsByModule, type Permission } from '$lib/api/roles';
	import {
		getDepartmentPermissions,
		updateDepartmentPermissions,
		type Department
	} from '$lib/api/staff';
	import { toast } from 'svelte-sonner';
	import { LoaderCircle, Shield, Layers } from 'lucide-svelte';

	let {
		open = $bindable(false),
		department,
		onSuccess
	} = $props<{
		department: Department | null;
		open: boolean;
		onSuccess?: () => void;
	}>();

	let permissionModules = $state<PermissionsByModule>({});
	let selectedPermissionIds = $state<Set<string>>(new Set());
	let loading = $state(false);
	let saving = $state(false);

	let moduleKeys = $derived(Object.keys(permissionModules).sort());

	$effect(() => {
		if (open && department) {
			loadData();
		}
	});

	async function loadData() {
		try {
			loading = true;
			// Load permissions and current access
			const [permResp, currentAccess] = await Promise.all([
				permissionAPI.listPermissionsByModule(),
				getDepartmentPermissions(department!.id)
			]);

			if (permResp.success && permResp.data) {
				permissionModules = permResp.data;
			}

			selectedPermissionIds = new Set(currentAccess);
		} catch (e) {
			toast.error('โหลดข้อมูลสิทธิ์ไม่สำเร็จ');
			console.error(e);
		} finally {
			loading = false;
		}
	}

	async function handleSave() {
		if (!department) return;
		try {
			saving = true;
			await updateDepartmentPermissions(department.id, Array.from(selectedPermissionIds));
			toast.success('บันทึกสิทธิ์การเข้าใช้งานสำเร็จ');
			open = false;
			onSuccess?.();
		} catch (e) {
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	function toggleModule(moduleName: string, select: boolean) {
		const perms = permissionModules[moduleName];
		if (!perms) return;

		const newSet = new Set(selectedPermissionIds);
		perms.forEach((p) => {
			if (select) newSet.add(p.id);
			else newSet.delete(p.id);
		});
		selectedPermissionIds = newSet;
	}

	function isModuleSelected(moduleName: string): boolean {
		const perms = permissionModules[moduleName];
		if (!perms || perms.length === 0) return false;
		return perms.every((p) => selectedPermissionIds.has(p.id));
	}

	function isModuleIndeterminate(moduleName: string): boolean {
		const perms = permissionModules[moduleName];
		if (!perms || perms.length === 0) return false;
		const selectedCount = perms.filter((p) => selectedPermissionIds.has(p.id)).length;
		return selectedCount > 0 && selectedCount < perms.length;
	}

	function togglePermission(permId: string, checked: boolean) {
		const newSet = new Set(selectedPermissionIds);
		if (checked) newSet.add(permId);
		else newSet.delete(permId);
		selectedPermissionIds = newSet;
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-[800px] max-h-[85vh] flex flex-col p-6">
		<Dialog.Header>
			<Dialog.Title class="text-xl flex items-center gap-2">
				<Shield class="w-5 h-5 text-primary" />
				กำหนดสิทธิ์การใช้งาน (Permissions)
			</Dialog.Title>
			<Dialog.Description>
				เลือกสิทธิ์การใช้งานระบบสำหรับฝ่าย <span class="font-bold text-foreground"
					>{department?.name}</span
				><br />
				<span class="text-xs text-muted-foreground"
					>* เมื่อได้รับสิทธิ์
					บุคลากรในฝ่ายจะสามารถเข้าถึงเมนูและใช้งานฟังก์ชันที่เกี่ยวข้องได้ทันที</span
				>
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex-1 overflow-y-auto py-4 pr-2 -mr-2">
			{#if loading}
				<div class="flex justify-center items-center py-20">
					<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
				</div>
			{:else}
				<div class="space-y-6">
					{#each moduleKeys as module}
						<div class="border rounded-lg p-4 bg-muted/20">
							<div class="flex items-center gap-2 mb-3 pb-2 border-b border-border/50">
								<Checkbox
									checked={isModuleSelected(module)}
									indeterminate={isModuleIndeterminate(module)}
									onCheckedChange={(c) => toggleModule(module, !!c)}
								/>
								<Layers class="w-4 h-4 text-primary" />
								<span class="font-semibold capitalize">{module} Module</span>
							</div>

							<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 pl-7">
								{#each permissionModules[module] as perm}
									<div
										class="flex items-start gap-2 p-2 rounded-md hover:bg-background border border-transparent hover:border-border transition-colors cursor-pointer group focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
										role="button"
										tabindex="0"
										onclick={() => togglePermission(perm.id, !selectedPermissionIds.has(perm.id))}
										onkeydown={(e) => {
											if (e.key === 'Enter' || e.key === ' ') {
												e.preventDefault();
												togglePermission(perm.id, !selectedPermissionIds.has(perm.id));
											}
										}}
									>
										<Checkbox
											class="mt-0.5"
											checked={selectedPermissionIds.has(perm.id)}
											onCheckedChange={(c) => togglePermission(perm.id, !!c)}
											onclick={(e) => e.stopPropagation()}
											tabindex={-1}
										/>
										<div class="flex flex-col">
											<span class="text-sm font-medium group-hover:text-primary transition-colors"
												>{perm.code}</span
											>
											{#if perm.description}
												<span
													class="text-[10px] text-muted-foreground line-clamp-2"
													title={perm.description}>{perm.description}</span
												>
											{/if}
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/each}

					{#if moduleKeys.length === 0}
						<div class="text-center py-10 text-muted-foreground">ไม่พบข้อมูลสิทธิ์ในระบบ</div>
					{/if}
				</div>
			{/if}
		</div>

		<Dialog.Footer class="pt-4 border-t mt-2">
			<Button variant="outline" onclick={() => (open = false)}>ยกเลิก</Button>
			<Button onclick={handleSave} disabled={saving}>
				{#if saving}
					<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />
				{/if}
				บันทึกสิทธิ์
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
