<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { permissionAPI, type PermissionsByModule } from '$lib/api/roles';
	import {
		permissionActionLabel,
		permissionScopeMeta,
		permissionScopeToneClass
	} from '$lib/permissions/registry';
	import {
		getOrganizationPermissions,
		updateOrganizationPermissions,
		type OrganizationUnit
	} from '$lib/api/staff';
	import { toast } from 'svelte-sonner';
	import { LoaderCircle, Shield, Layers, UsersRound } from 'lucide-svelte';
	import { SvelteSet } from 'svelte/reactivity';

	type PermissionPositionValue =
		| 'all'
		| 'director'
		| 'deputy_director'
		| 'head'
		| 'deputy_head'
		| 'coordinator'
		| 'member';

	type PermissionPositionColumn = {
		value: PermissionPositionValue;
		label: string;
		shortLabel: string;
	};

	const permissionPositionColumns: PermissionPositionColumn[] = [
		{ value: 'all', label: 'ทุกตำแหน่ง', shortLabel: 'ทุกคน' },
		{ value: 'director', label: 'ผู้อำนวยการ', shortLabel: 'ผอ.' },
		{ value: 'deputy_director', label: 'รองผู้อำนวยการ', shortLabel: 'รอง ผอ.' },
		{ value: 'head', label: 'หัวหน้า', shortLabel: 'หัวหน้า' },
		{ value: 'deputy_head', label: 'รองหัวหน้า', shortLabel: 'รองหัวหน้า' },
		{ value: 'coordinator', label: 'ผู้ประสานงาน', shortLabel: 'ประสาน' },
		{ value: 'member', label: 'สมาชิก', shortLabel: 'สมาชิก' }
	];

	let {
		open = $bindable(false),
		organizationUnit,
		onSuccess,
		readOnly = false
	} = $props<{
		organizationUnit: OrganizationUnit | null;
		open: boolean;
		onSuccess?: () => void;
		readOnly?: boolean;
	}>();

	let permissionModules = $state<PermissionsByModule>({});
	let selectedGrantKeys = new SvelteSet<string>();
	let loading = $state(false);
	let saving = $state(false);

	let moduleKeys = $derived(Object.keys(permissionModules).sort());
	let totalPermissionCount = $derived(
		Object.values(permissionModules).reduce((total, permissions) => total + permissions.length, 0)
	);

	$effect(() => {
		if (open && organizationUnit) {
			loadData();
		}
	});

	function grantKey(permissionId: string, position: PermissionPositionValue | null | undefined) {
		return `${permissionId}::${position ?? 'all'}`;
	}

	function normalizePositionCode(positionCode: string | null | undefined): PermissionPositionValue {
		if (permissionPositionColumns.some((column) => column.value === positionCode)) {
			return positionCode as PermissionPositionValue;
		}
		return 'all';
	}

	function parseGrantKey(key: string): {
		permission_id: string;
		position_code: Exclude<PermissionPositionValue, 'all'> | null;
	} {
		const [permission_id, rawPosition = 'all'] = key.split('::') as [
			string,
			PermissionPositionValue?
		];

		return {
			permission_id,
			position_code: rawPosition === 'all' ? null : rawPosition
		};
	}

	function hasGrant(permissionId: string, position: PermissionPositionValue) {
		return selectedGrantKeys.has(grantKey(permissionId, position));
	}

	function toggleGrant(permissionId: string, position: PermissionPositionValue, checked: boolean) {
		if (readOnly) return;
		const key = grantKey(permissionId, position);
		if (checked) selectedGrantKeys.add(key);
		else selectedGrantKeys.delete(key);
	}

	function positionGrantCount(position: PermissionPositionValue) {
		let count = 0;
		for (const key of selectedGrantKeys) {
			if (key.endsWith(`::${position}`)) count += 1;
		}
		return count;
	}

	function isModulePositionSelected(
		moduleName: string,
		position: PermissionPositionValue
	): boolean {
		const permissions = permissionModules[moduleName];
		if (!permissions || permissions.length === 0) return false;
		return permissions.every((permission) => hasGrant(permission.id, position));
	}

	function isModulePositionIndeterminate(
		moduleName: string,
		position: PermissionPositionValue
	): boolean {
		const permissions = permissionModules[moduleName];
		if (!permissions || permissions.length === 0) return false;
		const selectedCount = permissions.filter((permission) =>
			hasGrant(permission.id, position)
		).length;
		return selectedCount > 0 && selectedCount < permissions.length;
	}

	function toggleModulePosition(
		moduleName: string,
		position: PermissionPositionValue,
		checked: boolean
	) {
		if (readOnly) return;
		const permissions = permissionModules[moduleName];
		if (!permissions) return;

		for (const permission of permissions) {
			toggleGrant(permission.id, position, checked);
		}
	}

	async function loadData() {
		if (!organizationUnit) return;

		try {
			loading = true;
			const [permResp, currentAccess] = await Promise.all([
				permissionAPI.listPermissionsByModule(),
				getOrganizationPermissions(organizationUnit.id)
			]);

			if (permResp.success && permResp.data) {
				permissionModules = permResp.data;
			}

			selectedGrantKeys.clear();
			for (const grant of currentAccess) {
				selectedGrantKeys.add(
					grantKey(grant.permission_id, normalizePositionCode(grant.position_code))
				);
			}
		} catch (error) {
			toast.error('โหลดข้อมูลสิทธิ์ไม่สำเร็จ');
			console.error(error);
		} finally {
			loading = false;
		}
	}

	async function handleSave() {
		if (readOnly) return;
		if (!organizationUnit) return;

		try {
			saving = true;
			await updateOrganizationPermissions(
				organizationUnit.id,
				Array.from(selectedGrantKeys).map((key) => ({
					permission_id: parseGrantKey(key).permission_id,
					position_code: parseGrantKey(key).position_code
				}))
			);
			toast.success('บันทึกสิทธิ์การเข้าใช้งานสำเร็จ');
			open = false;
			onSuccess?.();
		} catch (error) {
			toast.error('บันทึกไม่สำเร็จ');
			console.error(error);
		} finally {
			saving = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="flex max-h-[88vh] flex-col p-0 sm:max-w-[1180px]">
		<Dialog.Header class="border-b px-6 py-5">
			<div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
				<div class="space-y-2">
					<Dialog.Title class="flex items-center gap-2 text-xl">
						<Shield class="h-5 w-5 text-primary" />
						สิทธิ์ตามตำแหน่ง
					</Dialog.Title>
					<Dialog.Description>
						<span class="font-medium text-foreground">{organizationUnit?.name}</span>
					</Dialog.Description>
				</div>

				<div class="grid grid-cols-2 gap-2 text-sm sm:grid-cols-4">
					<div class="rounded-md border bg-muted/30 px-3 py-2">
						<p class="text-xs text-muted-foreground">สิทธิ์ทั้งหมด</p>
						<p class="font-semibold">{totalPermissionCount}</p>
					</div>
					<div class="rounded-md border bg-muted/30 px-3 py-2">
						<p class="text-xs text-muted-foreground">รายการที่เลือก</p>
						<p class="font-semibold">{selectedGrantKeys.size}</p>
					</div>
					<div class="rounded-md border bg-muted/30 px-3 py-2">
						<p class="text-xs text-muted-foreground">ทุกตำแหน่ง</p>
						<p class="font-semibold">{positionGrantCount('all')}</p>
					</div>
					<div class="rounded-md border bg-muted/30 px-3 py-2">
						<p class="text-xs text-muted-foreground">เฉพาะตำแหน่ง</p>
						<p class="font-semibold">{selectedGrantKeys.size - positionGrantCount('all')}</p>
					</div>
				</div>
			</div>
		</Dialog.Header>

		<div class="flex-1 overflow-y-auto px-6 py-5">
			{#if loading}
				<div class="flex items-center justify-center py-20">
					<LoaderCircle class="h-8 w-8 animate-spin text-primary" />
				</div>
			{:else if moduleKeys.length === 0}
				<div class="rounded-lg border border-dashed py-12 text-center text-muted-foreground">
					ไม่พบข้อมูลสิทธิ์ในระบบ
				</div>
			{:else}
				<div class="space-y-5">
					<div class="flex flex-wrap gap-2">
						{#each permissionPositionColumns as column (column.value)}
							<div class="rounded-md border bg-background px-3 py-2 text-sm">
								<span class="text-muted-foreground">{column.shortLabel}</span>
								<span class="ml-2 font-semibold">{positionGrantCount(column.value)}</span>
							</div>
						{/each}
					</div>

					{#each moduleKeys as module (module)}
						<section class="overflow-hidden rounded-lg border bg-card">
							<div class="flex items-center gap-2 border-b bg-muted/20 px-4 py-3">
								<Layers class="h-4 w-4 text-primary" />
								<h3 class="font-semibold capitalize">{module}</h3>
								<span class="text-sm text-muted-foreground">
									{permissionModules[module]?.length ?? 0} สิทธิ์
								</span>
							</div>

							<div class="overflow-x-auto">
								<table class="w-full min-w-[1032px] table-fixed text-sm">
									<colgroup>
										<col class="w-[360px]" />
										{#each permissionPositionColumns as column (column.value)}
											<col class="w-[96px]" />
										{/each}
									</colgroup>
									<thead>
										<tr class="border-b bg-muted/10">
											<th class="px-4 py-3 text-left font-medium">Permission</th>
											{#each permissionPositionColumns as column (column.value)}
												<th class="px-2 py-3 font-medium">
													<div class="flex flex-col items-center gap-1">
														<span>{column.shortLabel}</span>
														<Checkbox
															aria-label={`${module} ${column.label}`}
															checked={isModulePositionSelected(module, column.value)}
															indeterminate={isModulePositionIndeterminate(module, column.value)}
															onCheckedChange={(checked) =>
																toggleModulePosition(module, column.value, !!checked)}
															disabled={readOnly}
														/>
													</div>
												</th>
											{/each}
										</tr>
									</thead>
									<tbody>
										{#each permissionModules[module] as permission (permission.id)}
											{@const scopeMeta = permissionScopeMeta(permission.scope)}
											<tr class="border-b last:border-b-0 hover:bg-muted/20">
												<td class="px-4 py-3 align-top">
													<div class="space-y-1">
														<div class="flex flex-wrap items-center gap-2">
															<p class="text-sm font-medium text-foreground">
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
														<p class="break-all font-mono text-xs font-medium text-foreground">
															{permission.code}
														</p>
														{#if permission.description}
															<p class="line-clamp-2 text-xs text-muted-foreground">
																{permission.description}
															</p>
														{/if}
														<p class="line-clamp-2 text-xs text-muted-foreground">
															{scopeMeta.description}
														</p>
													</div>
												</td>
												{#each permissionPositionColumns as column (column.value)}
													<td class="px-2 py-3 align-top">
														<div class="flex justify-center">
															<Checkbox
																aria-label={`${permission.code} ${column.label}`}
																checked={hasGrant(permission.id, column.value)}
																onCheckedChange={(checked) =>
																	toggleGrant(permission.id, column.value, !!checked)}
																disabled={readOnly}
															/>
														</div>
													</td>
												{/each}
											</tr>
										{/each}
									</tbody>
								</table>
							</div>
						</section>
					{/each}
				</div>
			{/if}
		</div>

		<Dialog.Footer class="border-t px-6 py-4">
			<div class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
				<div class="flex items-center gap-2 text-xs text-muted-foreground">
					<UsersRound class="h-4 w-4" />
					<span>บันทึกตามตำแหน่งในหน่วยงาน</span>
				</div>
				<div class="flex justify-end gap-2">
					<Button variant="outline" onclick={() => (open = false)}>
						{readOnly ? 'ปิด' : 'ยกเลิก'}
					</Button>
					{#if !readOnly}
						<Button onclick={handleSave} disabled={saving}>
							{#if saving}
								<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
							{/if}
							บันทึกสิทธิ์
						</Button>
					{/if}
				</div>
			</div>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
