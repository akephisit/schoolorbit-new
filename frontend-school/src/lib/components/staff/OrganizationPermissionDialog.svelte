<script lang="ts">
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { filterOrganizationPermissions } from '#lib/features/staff/organization-permission-search.js';
	import { permissionAPI, type PermissionsByModule } from '#lib/api/roles.js';
	import {
		permissionActionLabel,
		permissionModuleLabel,
		permissionScopeMeta,
		permissionScopeToneClass
	} from '#lib/permissions/registry.js';
	import {
		getOrganizationPermissions,
		updateOrganizationPermissions,
		type OrganizationUnit
	} from '#lib/api/staff.js';
	import { toast } from 'svelte-sonner';
	import { LoaderCircle, Shield, Layers, UsersRound } from '@lucide/svelte';
	import { onDestroy, untrack } from 'svelte';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { requireApiData } from '#lib/api/client.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { SvelteSet } from 'svelte/reactivity';

	type PermissionPositionValue =
		'all' | 'director' | 'deputy_director' | 'head' | 'deputy_head' | 'coordinator' | 'member';

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
	}: {
		organizationUnit:
			(Pick<OrganizationUnit, 'id'> & Partial<Pick<OrganizationUnit, 'name'>>) | null;
		open: boolean;
		onSuccess?: () => void;
		readOnly?: boolean;
	} = $props();

	let permissionModules = $state<PermissionsByModule>({});
	let selectedGrantKeys = new SvelteSet<string>();
	let catalogLoading = $state(false),
		grantsLoading = $state(false),
		catalogLoaded = $state(false),
		grantsLoaded = $state(false),
		catalogError = $state(''),
		grantsError = $state('');
	const loading = $derived(catalogLoading || grantsLoading);
	const catalogRequest = new LatestRequest(),
		grantsRequest = new LatestRequest();
	const canReadCatalog = $derived($can.has(PERMISSIONS.SETTINGS_READ_ALL));
	const canReadGrants = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const canSave = $derived(!readOnly && $can.has(PERMISSIONS.ROLES_UPDATE_ALL));
	let disposed = false,
		ownerEpoch = 0;
	let saving = $state(false);
	let saveError = $state('');
	let search = $state('');
	let mobilePosition = $state<PermissionPositionValue>('head');
	const visiblePermissionModules = $derived(
		filterOrganizationPermissions(permissionModules, search)
	);
	const visiblePermissionCount = $derived(
		Object.values(visiblePermissionModules).reduce(
			(sum, permissions) => sum + permissions.length,
			0
		)
	);

	let moduleKeys = $derived(
		Object.keys(visiblePermissionModules).sort((a, b) =>
			permissionModuleLabel(a).localeCompare(permissionModuleLabel(b), 'th')
		)
	);
	let totalPermissionCount = $derived(
		Object.values(permissionModules).reduce((total, permissions) => total + permissions.length, 0)
	);

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
		if (!canSave || saving || loading || !grantsLoaded || !catalogLoaded) return;
		const key = grantKey(permissionId, position);
		if (checked) selectedGrantKeys.add(key);
		else selectedGrantKeys.delete(key);
	}

	function isModulePositionSelected(
		moduleName: string,
		position: PermissionPositionValue
	): boolean {
		const permissions = visiblePermissionModules[moduleName];
		if (!permissions || permissions.length === 0) return false;
		return permissions.every((permission) => hasGrant(permission.id, position));
	}

	function isModulePositionIndeterminate(
		moduleName: string,
		position: PermissionPositionValue
	): boolean {
		const permissions = visiblePermissionModules[moduleName];
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
		if (!canSave || saving || loading || !grantsLoaded || !catalogLoaded) return;
		const permissions = visiblePermissionModules[moduleName];
		if (!permissions) return;

		for (const permission of permissions) {
			toggleGrant(permission.id, position, checked);
		}
	}

	async function loadCatalog() {
		if (!open || !canReadCatalog) return;
		const ticket = catalogRequest.begin();
		catalogLoading = true;
		catalogError = '';
		const result = await captureRouteLoad(
			permissionAPI
				.listPermissionsByModule({ signal: ticket.signal })
				.then((reply) => requireApiData(reply, 'โหลดรายการสิทธิ์ไม่สำเร็จ')),
			'โหลดรายการสิทธิ์ไม่สำเร็จ'
		);
		if (!catalogRequest.isCurrent(ticket.revision)) return;
		catalogLoading = false;
		if (result.ok) {
			permissionModules = result.data;
			catalogLoaded = true;
		} else catalogError = result.error;
	}
	async function loadGrants() {
		if (!open || !organizationUnit || !canReadGrants) return;
		const ticket = grantsRequest.begin();
		grantsLoading = true;
		grantsError = '';
		const result = await captureRouteLoad(
			getOrganizationPermissions(organizationUnit.id, { signal: ticket.signal }),
			'โหลดสิทธิ์หน่วยงานไม่สำเร็จ'
		);
		if (!grantsRequest.isCurrent(ticket.revision)) return;
		grantsLoading = false;
		if (result.ok) {
			selectedGrantKeys.clear();
			for (const grant of result.data)
				selectedGrantKeys.add(
					grantKey(grant.permission_id, normalizePositionCode(grant.position_code))
				);
			grantsLoaded = true;
		} else grantsError = result.error;
	}
	async function handleSave() {
		if (
			!open ||
			!organizationUnit ||
			!canSave ||
			saving ||
			!grantsLoaded ||
			!catalogLoaded ||
			loading ||
			grantsError ||
			catalogError
		)
			return;
		const owner = organizationUnit.id,
			epoch = ownerEpoch;
		const current = () => !disposed && epoch === ownerEpoch && organizationUnit?.id === owner;
		grantsRequest.abort();
		saving = true;
		saveError = '';
		try {
			const res = await updateOrganizationPermissions(
				owner,
				Array.from(selectedGrantKeys).map((key) => parseGrantKey(key))
			);
			if (!current()) return;
			if (!res.success) throw new Error(res.error ?? 'บันทึกสิทธิ์ไม่สำเร็จ');
			toast.success('บันทึกสิทธิ์การเข้าใช้งานสำเร็จ');
			open = false;
			onSuccess?.();
		} catch (error) {
			if (current()) saveError = error instanceof Error ? error.message : 'บันทึกสิทธิ์ไม่สำเร็จ';
		} finally {
			if (current()) saving = false;
		}
	}
	$effect.pre(() => {
		const id = organizationUnit?.id,
			visible = open,
			readGrants = canReadGrants,
			readCatalog = canReadCatalog;
		untrack(() => {
			ownerEpoch++;
			search = '';
			saveError = '';
			mobilePosition = 'head';
			saving = false;
			permissionModules = {};
			selectedGrantKeys.clear();
			catalogLoaded = false;
			grantsLoaded = false;
			catalogLoading = false;
			grantsLoading = false;
			catalogError = '';
			grantsError = '';
			if (visible && id) {
				if (readGrants) void loadGrants();
				if (readCatalog) void loadCatalog();
			}
		});
		return () => {
			catalogRequest.abort();
			grantsRequest.abort();
		};
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		catalogRequest.abort();
		grantsRequest.abort();
	});
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="flex max-h-[90dvh] min-h-0 flex-col gap-0 p-0 sm:max-w-[1180px]">
		<Dialog.Header
			class="border-b bg-gradient-to-br from-primary/5 via-card to-card px-4 py-4 pr-10 sm:px-6"
		>
			<div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
				<div class="space-y-2">
					<Dialog.Title class="flex items-center gap-2 text-xl">
						<Shield class="h-5 w-5 text-primary" />
						สิทธิ์ตามตำแหน่ง
					</Dialog.Title>
					<Dialog.Description>
						<span class="break-words font-medium text-foreground">{organizationUnit?.name}</span>
					</Dialog.Description>
				</div>
			</div>
		</Dialog.Header>

		<div class="space-y-3 border-b px-4 py-4 sm:px-6">
			<p class="text-sm text-muted-foreground">
				เลือกสิทธิ์ให้แต่ละตำแหน่งในหน่วยงานนี้ · “ทุกตำแหน่ง” ใช้กับสมาชิกทุกคน
			</p>
			<div class="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
				<div class="space-y-2">
					<Label for="organization-permission-search">ค้นหาสิทธิ์</Label>
					<Input
						id="organization-permission-search"
						bind:value={search}
						placeholder="ชื่อสิทธิ์ หมวดงาน หรือขอบเขต เช่น บุคลากร ทั้งโรงเรียน"
						disabled={!catalogLoaded}
					/>
				</div>
				<div class="space-y-2 md:hidden">
					<Label for="organization-permission-position">ตำแหน่งที่ต้องการกำหนด</Label>
					<Select.Root type="single" bind:value={mobilePosition}>
						<Select.Trigger id="organization-permission-position" class="w-full"
							>{permissionPositionColumns.find((column) => column.value === mobilePosition)
								?.label}</Select.Trigger
						>
						<Select.Content
							>{#each permissionPositionColumns as column (column.value)}<Select.Item
									value={column.value}>{column.label}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
				</div>
				<p class="text-sm text-muted-foreground" aria-live="polite">
					{catalogLoaded
						? `${visiblePermissionCount} จาก ${totalPermissionCount} สิทธิ์`
						: 'กำลังโหลดรายการสิทธิ์'}
				</p>
			</div>
		</div>
		<div class="min-h-0 flex-1 overflow-y-auto px-4 py-4 sm:px-6">
			<section data-testid="organization-grants" aria-busy={grantsLoading}>
				{#if !canReadGrants}<PageState variant="permission" title="ไม่มีสิทธิ์อ่านสิทธิ์หน่วยงาน" />
				{:else if grantsLoading && !grantsLoaded}<div
						role="status"
						aria-label="กำลังโหลดสิทธิ์หน่วยงาน"
					>
						<PageSkeleton variant="cards" rows={1} />
					</div>
				{/if}
				{#if grantsError}<PageState
						variant="error"
						title="โหลดสิทธิ์หน่วยงานไม่สำเร็จ"
						description={grantsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadGrants}
					/>{/if}
			</section>
			<section data-testid="organization-permission-catalog" aria-busy={catalogLoading}>
				{#if !canReadCatalog}<PageState
						variant="permission"
						title="ไม่มีสิทธิ์อ่านรายการสิทธิ์"
					/>{/if}
				{#if catalogError}<PageState
						variant="error"
						title="โหลดรายการสิทธิ์ไม่สำเร็จ"
						description={catalogError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadCatalog}
					/>{/if}
				{#if catalogLoading && !catalogLoaded}<div role="status" aria-label="กำลังโหลดรายการสิทธิ์">
						<PageSkeleton variant="cards" rows={1} />
					</div>{/if}
			</section>
			{#if !canReadCatalog || !canReadGrants || catalogError || grantsError}
				<p class="text-sm text-muted-foreground">ยังไม่สามารถแก้ไขสิทธิ์ได้</p>
			{:else if loading}
				<div class="flex items-center justify-center py-20">
					<LoaderCircle class="h-8 w-8 animate-spin text-primary" />
				</div>
			{:else if totalPermissionCount === 0}
				<div class="rounded-lg border border-dashed py-12 text-center text-muted-foreground">
					ไม่พบข้อมูลสิทธิ์ในระบบ
				</div>
			{:else}
				<div class="space-y-5">
					{#if moduleKeys.length === 0}<PageState
							title="ไม่พบสิทธิ์ที่ตรงคำค้น"
							description="ลองค้นหาด้วยชื่อหมวดงานหรือชื่อสิทธิ์อื่น สิทธิ์ที่เลือกไว้ยังคงอยู่"
							actionLabel="ล้างคำค้น"
							onaction={() => (search = '')}
						/>{/if}

					{#each moduleKeys as module (module)}
						<section class="overflow-hidden rounded-lg border bg-card">
							<div class="flex items-center gap-2 border-b bg-muted/20 px-4 py-3">
								<Layers class="h-4 w-4 text-primary" />
								<h3 class="font-semibold">{permissionModuleLabel(module)}</h3>
								<span class="text-sm text-muted-foreground">
									{visiblePermissionModules[module]?.length ?? 0} สิทธิ์
								</span>
							</div>

							<div class="min-w-0">
								<Table.Root class="w-full table-fixed text-sm md:min-w-[1032px]">
									<colgroup>
										<col class="md:w-[360px]" />
										{#each permissionPositionColumns as column (column.value)}
											<col
												class={column.value === mobilePosition
													? 'w-20 md:w-[96px]'
													: 'hidden w-[96px] md:table-column'}
											/>
										{/each}
									</colgroup>
									<Table.Header>
										<Table.Row class="border-b bg-muted/10">
											<Table.Head class="whitespace-normal px-4 py-3 text-left font-medium"
												>สิทธิ์และขอบเขต<span
													class="mt-1 block text-xs font-normal text-muted-foreground"
													>ช่องหัวตารางเลือกเฉพาะสิทธิ์ที่แสดง</span
												></Table.Head
											>
											{#each permissionPositionColumns as column (column.value)}
												<Table.Head
													class={`px-2 py-3 font-medium ${column.value === mobilePosition ? '' : 'hidden md:table-cell'}`}
												>
													<div class="flex flex-col items-center gap-1">
														<span>{column.shortLabel}</span>
														<Checkbox
															aria-label={`เลือกสิทธิ์ที่แสดงในหมวด${permissionModuleLabel(module)} · ${column.label}`}
															checked={isModulePositionSelected(module, column.value)}
															indeterminate={isModulePositionIndeterminate(module, column.value)}
															onCheckedChange={(checked) =>
																toggleModulePosition(module, column.value, !!checked)}
															disabled={!canSave || !grantsLoaded || loading || saving}
														/>
													</div>
												</Table.Head>
											{/each}
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#each visiblePermissionModules[module] as permission (permission.id)}
											{@const scopeMeta = permissionScopeMeta(permission.scope)}
											<Table.Row class="border-b last:border-b-0 hover:bg-muted/20">
												<Table.Cell class="whitespace-normal px-4 py-3 align-top">
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
														{#if permission.description}
															<p class="text-xs text-muted-foreground">
																{permission.description}
															</p>
														{/if}
														<p class="text-xs text-muted-foreground">
															{scopeMeta.description}
														</p>
													</div>
												</Table.Cell>
												{#each permissionPositionColumns as column (column.value)}
													<Table.Cell
														class={`px-2 py-3 align-top ${column.value === mobilePosition ? '' : 'hidden md:table-cell'}`}
													>
														<div class="flex justify-center">
															<Checkbox
																aria-label={`${permission.name} · ${scopeMeta.label} · ${column.label}`}
																checked={hasGrant(permission.id, column.value)}
																onCheckedChange={(checked) =>
																	toggleGrant(permission.id, column.value, !!checked)}
																disabled={!canSave || !grantsLoaded || loading || saving}
															/>
														</div>
													</Table.Cell>
												{/each}
											</Table.Row>
										{/each}
									</Table.Body>
								</Table.Root>
							</div>
						</section>
					{/each}
				</div>
			{/if}
		</div>

		{#if saveError}<p role="alert" class="px-4 py-3 text-sm text-destructive sm:px-6">
				{saveError}
			</p>{/if}
		<Dialog.Footer class="border-t px-4 py-4 sm:px-6">
			<div class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
				<div class="flex items-center gap-2 text-xs text-muted-foreground">
					<UsersRound class="h-4 w-4" />
					<span
						>{grantsLoaded
							? `เลือกไว้ ${selectedGrantKeys.size} รายการ · รวมรายการที่ซ่อนจากการค้นหา`
							: 'กำลังโหลดสิทธิ์ที่บันทึกไว้'}</span
					>
				</div>
				<div class="flex justify-end gap-2">
					<Button variant="outline" onclick={() => (open = false)}>
						{readOnly ? 'ปิด' : 'ยกเลิก'}
					</Button>
					{#if canSave}
						<Button
							onclick={handleSave}
							disabled={saving ||
								loading ||
								!grantsLoaded ||
								!catalogLoaded ||
								!!grantsError ||
								!!catalogError}
						>
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
