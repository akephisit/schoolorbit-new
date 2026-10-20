<script lang="ts">
	import { onDestroy, tick, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import {
		organizationPeople,
		type OrganizationPerson
	} from '#lib/features/staff/organization-members.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { requireApiData } from '#lib/api/client.js';
	import { organizationDetailHref } from '#lib/navigation/organization.js';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import OrganizationHierarchy from '#lib/components/staff/OrganizationHierarchy.svelte';
	import {
		organizationHierarchy,
		organizationAncestors,
		organizationUnitLabel as unitTypeLabel,
		type UnitTypeFilter
	} from '#lib/features/staff/organization-hierarchy.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import LearningAreaWorkspace from '#lib/components/staff/LearningAreaWorkspace.svelte';
	import {
		deleteOrganizationUnit,
		deleteOrganizationUnitPermanently,
		listOrganizationMembers,
		listOrganizationUnits,
		updateOrganizationUnit
	} from '#lib/api/staff.js';
	import type { OrganizationUnit } from '#lib/api/staff.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '#lib/components/ui/dialog/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import {
		ArrowRight,
		Building2,
		KeyRound,
		ChevronRight,
		ArrowLeft,
		Pencil,
		Plus,
		Power,
		Trash2,
		RotateCcw,
		Search,
		Users
	} from '@lucide/svelte';
	import OrganizationUnitDialog from '#lib/components/staff/OrganizationUnitDialog.svelte';
	import OrganizationPermissionDialog from '#lib/components/staff/OrganizationPermissionDialog.svelte';
	import { toast } from 'svelte-sonner';

	const unitTypeFilters: { value: UnitTypeFilter; label: string }[] = [
		{ value: 'all', label: 'ทั้งหมด' },
		{ value: 'management_group', label: 'กลุ่มบริหาร' },
		{ value: 'division', label: 'ฝ่าย/งาน' },
		{ value: 'other', label: 'อื่น ๆ' }
	];

	let departments: OrganizationUnit[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	const currentUrl = $derived(page.shallow?.url ?? page.url);
	const searchQuery = $derived(currentUrl.searchParams.get('q') ?? '');
	const activeUnitTypeFilter = $derived(
		unitTypeFilters.find((filter) => filter.value === currentUrl.searchParams.get('type'))?.value ??
			'all'
	);
	let detailHeading: HTMLHeadingElement | undefined = $state();
	let hierarchyHeading: HTMLHeadingElement | undefined = $state();
	let workspaceToolbar: HTMLDivElement | undefined = $state();
	let createParentId: string | undefined = $state();
	const selectedUnitId = $derived(currentUrl.searchParams.get('unit'));
	const canReadGroups = $derived(
		$can.hasAny(
			PERMISSIONS.ROLES_READ_ALL,
			PERMISSIONS.ACADEMIC_CATALOG_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL
		)
	);

	let showDialog = $state(false);
	let editingDepartment: OrganizationUnit | null = $state(null);
	let showPermissionDialog = $state(false);
	let permissionDepartment = $state<OrganizationUnit | null>(null);
	let showDeactivateDialog = $state(false);
	let statusMutationPending = $state(false);
	let deletingUnit = $state<OrganizationUnit | null>(null);
	let permanentDeleteError = $state('');
	let permanentDeletePending = $state(false);

	let selectedMembers = $state<OrganizationPerson[]>([]);
	let selectedMembersLoading = $state(false);
	let selectedMembersUnitId = $state('');
	let selectedMembersError = $state(''),
		membersLoaded = $state(false),
		unitsLoaded = $state(false);
	let { data }: PageProps = $props();
	const unitsSource = $derived(data.units),
		membersSource = $derived(data.members);
	const unitsRequest = new LatestRequest(),
		membersRequest = new LatestRequest();
	let disposed = false,
		initialMemberOwner = '',
		initialMembersPending = false;

	const canReadOrganization = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const activeSection = $derived(
		data.tab === 'subject-groups' || !canReadOrganization ? 'subject-groups' : 'administration'
	);
	const canCreateOrganizationUnit = $derived($can.has(PERMISSIONS.ROLES_CREATE_ALL));
	const canUpdateOrganizationUnit = $derived($can.has(PERMISSIONS.ROLES_UPDATE_ALL));
	const canDeleteOrganizationUnit = $derived($can.has(PERMISSIONS.ROLES_DELETE_ALL));
	const canReadOrganizationPermissions = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const canUpdateOrganizationPermissions = $derived($can.has(PERMISSIONS.ROLES_UPDATE_ALL));
	const canAssignOrganizationMembers = $derived($can.has(PERMISSIONS.ROLES_ASSIGN_ALL));

	let organizationStats = $derived.by(() => ({
		total: departments.length,
		managementGroups: departments.filter((unit) => unit.unit_type === 'management_group').length,
		divisions: departments.filter((unit) => unit.unit_type === 'division').length,
		active: departments.filter((unit) => unit.is_active).length
	}));

	const visibleTreeRoots = $derived(
		organizationHierarchy(departments, searchQuery, activeUnitTypeFilter)
	);
	let selectedUnit = $derived.by(() => {
		return (
			departments.find((unit) => unit.id === selectedUnitId) ??
			departments.find((unit) => isSchoolRoot(unit)) ??
			null
		);
	});

	let selectedUnitChildren = $derived.by(() =>
		selectedUnit ? getChildren(selectedUnit.id, departments) : []
	);

	const selectedUnitAncestors = $derived(
		selectedUnit ? organizationAncestors(selectedUnit, departments) : []
	);
	const selectedManagementGroup = $derived(
		selectedUnitAncestors.findLast((unit) => unit.unit_type === 'management_group')
	);

	function getChildren(parentId: string, source = departments): OrganizationUnit[] {
		return source
			.filter((unit) => unit.parent_unit_id === parentId)
			.sort((a, b) => (a.display_order || 0) - (b.display_order || 0));
	}

	function isSchoolRoot(unit: OrganizationUnit): boolean {
		return unit.code === 'SCHOOL' || unit.unit_type === 'school';
	}

	function categoryLabel(unit: OrganizationUnit): string {
		if (unit.category === 'academic') return 'วิชาการ';
		if (unit.category === 'student_affairs') return 'กิจการนักเรียน';
		if (unit.category === 'personnel') return 'บุคคล';
		if (unit.category === 'budget') return 'งบประมาณ';
		if (unit.category === 'administrative') return 'บริหาร';
		return 'ทั่วไป';
	}

	function positionLabel(positionCode: string): string {
		if (positionCode === 'director') return 'ผู้อำนวยการ';
		if (positionCode === 'deputy_director') return 'รองผู้อำนวยการ';
		if (positionCode === 'head') return 'หัวหน้า';
		if (positionCode === 'deputy_head') return 'รองหัวหน้า';
		if (positionCode === 'coordinator') return 'ผู้ประสานงาน';
		return 'สมาชิก';
	}

	function applyUnits(result: Awaited<typeof data.units>, revision: number) {
		if (!unitsRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			departments = result.data.filter((unit) => unit.unit_type !== 'subject_group');
			unitsLoaded = true;
		} else error = result.error;
	}
	async function loadDepartments() {
		if (!canReadOrganization || activeSection !== 'administration') return;
		const ticket = unitsRequest.begin();
		loading = true;
		error = '';
		applyUnits(
			await captureRouteLoad(
				listOrganizationUnits({ include_inactive: true }, { signal: ticket.signal }).then((reply) =>
					requireApiData(reply, 'โหลดหน่วยงานไม่สำเร็จ')
				),
				'โหลดหน่วยงานไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	async function loadSelectedMembers(unitId: string) {
		if (!canReadOrganization || activeSection !== 'administration' || !unitId) return;
		const ticket = membersRequest.begin();
		if (selectedMembersUnitId !== unitId) {
			selectedMembers = [];
			membersLoaded = false;
		}
		selectedMembersUnitId = unitId;
		selectedMembersLoading = true;
		selectedMembersError = '';
		const result = await captureRouteLoad(
			listOrganizationMembers(unitId, { include_children: true }, { signal: ticket.signal }).then(
				(reply) => requireApiData(reply, 'โหลดสมาชิกไม่สำเร็จ')
			),
			'โหลดสมาชิกไม่สำเร็จ'
		);
		if (!membersRequest.isCurrent(ticket.revision) || selectedUnit?.id !== unitId) return;
		selectedMembersLoading = false;
		if (result.ok) {
			selectedMembers = organizationPeople(result.data, unitId);
			membersLoaded = true;
		} else selectedMembersError = result.error;
	}

	function handleCreate(parent?: OrganizationUnit) {
		if (!canCreateOrganizationUnit) return;
		editingDepartment = null;
		createParentId = parent?.id;
		showDialog = true;
	}

	function handleEdit(unit: OrganizationUnit) {
		if (!canUpdateOrganizationUnit) return;
		editingDepartment = unit;
		showDialog = true;
	}

	function sectionHref(section: 'administration' | 'subject-groups') {
		const url = new URL(currentUrl.href);
		url.searchParams.set('tab', section);
		return url.pathname + url.search;
	}

	async function handleSelectUnit(unitId: string) {
		const url = new URL(currentUrl.href);
		url.searchParams.set('unit', unitId);
		await goto(url.pathname + url.search, { shallow: true, reset: false });
		await tick();
		if (window.matchMedia('(max-width: 1023px)').matches) {
			detailHeading?.focus({ preventScroll: true });
			workspaceToolbar?.scrollIntoView({ block: 'start' });
		}
	}
	async function showHierarchy() {
		const url = new URL(currentUrl.href);
		url.searchParams.delete('unit');
		await goto(url.pathname + url.search, { shallow: true, reset: false });
		await tick();
		hierarchyHeading?.focus({ preventScroll: true });
		hierarchyHeading?.scrollIntoView({ block: 'start' });
	}
	function filterHierarchy(query: string, type: UnitTypeFilter) {
		const url = new URL(currentUrl.href);
		if (query) url.searchParams.set('q', query);
		else url.searchParams.delete('q');
		if (type !== 'all') url.searchParams.set('type', type);
		else url.searchParams.delete('type');
		void goto(url.pathname + url.search, {
			shallow: true,
			replace: true,
			reset: false
		});
	}

	function handlePermission(unit: OrganizationUnit) {
		if (!canReadOrganizationPermissions) return;
		permissionDepartment = unit;
		showPermissionDialog = true;
	}

	async function handlePermanentDelete() {
		const unit = deletingUnit;
		if (
			!unit ||
			!canDeleteOrganizationUnit ||
			unit.is_system ||
			unit.unit_type !== 'management_group' ||
			unit.subject_group_id ||
			permanentDeletePending
		)
			return;
		unitsRequest.abort();
		loading = false;
		permanentDeletePending = true;
		permanentDeleteError = '';
		try {
			const response = await deleteOrganizationUnitPermanently(unit.id);
			if (disposed) return;
			if (!response.success) throw new Error(response.error || 'ลบกลุ่มบริหารไม่สำเร็จ');
			departments = departments.filter((item) => item.id !== unit.id);
			deletingUnit = null;
			toast.success('ลบกลุ่มบริหารสำเร็จ');
			if (selectedUnit?.id === unit.id || selectedUnitId === unit.id) await showHierarchy();
		} catch (cause) {
			if (!disposed)
				permanentDeleteError = cause instanceof Error ? cause.message : 'ลบกลุ่มบริหารไม่สำเร็จ';
		} finally {
			if (!disposed) permanentDeletePending = false;
		}
	}

	async function handleDeactivateOrganizationUnit() {
		const unit = selectedUnit;
		if (!unit || !canDeleteOrganizationUnit) {
			toast.error('ไม่มีสิทธิ์ปิดใช้งานหน่วยงาน');
			return;
		}
		if (unit.is_system) {
			toast.error('ไม่สามารถปิดใช้งานหน่วยงานระบบได้');
			return;
		}

		if (statusMutationPending) return;
		unitsRequest.abort();
		loading = false;
		statusMutationPending = true;
		try {
			const response = await deleteOrganizationUnit(unit.id);
			if (disposed) return;
			if (response.success) {
				toast.success('ปิดใช้งานหน่วยงานสำเร็จ');
				if (selectedUnit?.id === unit.id) showDeactivateDialog = false;
				await loadDepartments();
			} else {
				toast.error(response.error || 'ไม่สามารถปิดใช้งานหน่วยงานได้');
			}
		} catch (cause) {
			if (disposed) return;
			console.error('Failed to deactivate organization unit:', cause);
			toast.error(cause instanceof Error ? cause.message : 'ไม่สามารถปิดใช้งานหน่วยงานได้');
		} finally {
			if (!disposed) statusMutationPending = false;
		}
	}

	async function handleReactivateOrganizationUnit() {
		const unit = selectedUnit;
		if (!unit || !canUpdateOrganizationUnit) {
			toast.error('ไม่มีสิทธิ์เปิดใช้งานหน่วยงาน');
			return;
		}

		if (statusMutationPending) return;
		unitsRequest.abort();
		loading = false;
		statusMutationPending = true;
		try {
			const response = await updateOrganizationUnit(unit.id, { is_active: true });
			if (disposed) return;
			if (response.success) {
				toast.success('เปิดใช้งานหน่วยงานสำเร็จ');
				await loadDepartments();
			} else {
				toast.error(response.error || 'ไม่สามารถเปิดใช้งานหน่วยงานได้');
			}
		} catch (cause) {
			if (disposed) return;
			console.error('Failed to reactivate organization unit:', cause);
			toast.error(cause instanceof Error ? cause.message : 'ไม่สามารถเปิดใช้งานหน่วยงานได้');
		} finally {
			if (!disposed) statusMutationPending = false;
		}
	}

	$effect.pre(() => {
		const source = unitsSource,
			memberRead = membersSource,
			active = activeSection === 'administration';
		untrack(() => {
			if (!active) {
				showDialog = false;
				showPermissionDialog = false;
				showDeactivateDialog = false;
				unitsRequest.abort();
				membersRequest.abort();
				return;
			}
			const ticket = unitsRequest.begin(),
				memberTicket = membersRequest.begin();
			loading = true;
			error = '';
			selectedMembersLoading = true;
			selectedMembersError = '';
			initialMembersPending = true;
			void source.then((result) => {
				if (!unitsRequest.isCurrent(ticket.revision)) return;
				applyUnits(result, ticket.revision);
				initialMemberOwner = result.ok
					? ((
							result.data.find((unit) => unit.id === selectedUnitId) ??
							result.data.find(isSchoolRoot) ??
							result.data[0]
						)?.id ?? '')
					: '';
				if (selectedUnit?.id === initialMemberOwner) selectedMembersUnitId = initialMemberOwner;
				else if (selectedUnit) void loadSelectedMembers(selectedUnit.id);
			});
			void memberRead.then((result) => {
				if (!membersRequest.isCurrent(memberTicket.revision)) return;
				initialMembersPending = false;
				selectedMembersLoading = false;
				if (!result.ok) selectedMembersError = result.error;
				else if (result.data.unitId === selectedUnit?.id) {
					selectedMembers = organizationPeople(result.data.members, result.data.unitId);
					membersLoaded = true;
					selectedMembersUnitId = result.data.unitId;
				}
			});
		});
		return () => {
			unitsRequest.abort();
			membersRequest.abort();
		};
	});
	$effect.pre(() => {
		const id = selectedUnit?.id ?? '',
			allowed = canReadOrganization,
			active = activeSection === 'administration';
		untrack(() => {
			if (!allowed) {
				departments = [];
				unitsLoaded = false;
				membersRequest.abort();
				selectedMembers = [];
				membersLoaded = false;
				selectedMembersLoading = false;
				selectedMembersUnitId = '';
				return;
			}
			if (!active) {
				membersRequest.abort();
				selectedMembersLoading = false;
				return;
			}
			if (!id) return;
			if (selectedMembersUnitId !== id) {
				membersRequest.abort();
				selectedMembers = [];
				membersLoaded = false;
				selectedMembersError = '';
				selectedMembersLoading = true;
				if (initialMembersPending && id === initialMemberOwner) selectedMembersUnitId = id;
				else void loadSelectedMembers(id);
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		unitsRequest.abort();
		membersRequest.abort();
	});
</script>

<PageShell
	title="โครงสร้างโรงเรียน"
	description="ดูกลุ่มบริหาร ฝ่าย/งาน และกลุ่มสาระการเรียนรู้"
	contentClass="flex flex-col gap-4 [&>*]:mt-0"
>
	<nav aria-label="ประเภทโครงสร้าง" class="flex flex-wrap gap-2">
		{#if canReadOrganization}<Button
				variant={activeSection === 'administration' ? 'default' : 'outline'}
				href={sectionHref('administration')}
				aria-current={activeSection === 'administration' ? 'page' : undefined}
				>กลุ่มบริหารและฝ่าย/งาน</Button
			>{/if}
		{#if canReadGroups}<Button
				variant={activeSection === 'subject-groups' ? 'default' : 'outline'}
				href={sectionHref('subject-groups')}
				aria-current={activeSection === 'subject-groups' ? 'page' : undefined}
				>กลุ่มสาระการเรียนรู้</Button
			>{/if}
	</nav>
	{#if activeSection === 'subject-groups'}
		<LearningAreaWorkspace source={data.groups} />
	{:else if !canReadOrganization}
		<PageState variant="permission" title="ไม่มีสิทธิ์ดูโครงสร้างโรงเรียน" />
	{:else}
		<section data-testid="organization-catalog" aria-busy={loading} class="space-y-4">
			<div
				bind:this={workspaceToolbar}
				class="flex scroll-mt-20 flex-wrap items-start justify-between gap-3"
				data-testid="organization-toolbar"
			>
				<div class={selectedUnitId ? 'hidden lg:block' : ''}>
					<h2
						bind:this={hierarchyHeading}
						tabindex="-1"
						class="scroll-mt-20 font-semibold outline-none"
					>
						เลือกหน่วยงาน
					</h2>
					{#if unitsLoaded}<p class="mt-1 text-sm text-muted-foreground">
							{organizationStats.managementGroups} กลุ่มบริหาร · {organizationStats.divisions} ฝ่าย/งาน
						</p>{/if}
				</div>
				{#if selectedUnitId}<Button variant="outline" onclick={showHierarchy} class="lg:hidden"
						><ArrowLeft class="size-4" />กลับไปเลือกหน่วยงาน</Button
					>{/if}
				{#if canCreateOrganizationUnit}<Button onclick={() => handleCreate()}
						><Plus class="size-4" />เพิ่มหน่วยงาน</Button
					>{/if}
			</div>
			{#if error && unitsLoaded}<PageState
					variant="error"
					title="อัปเดตโครงสร้างไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadDepartments}
				/>{/if}
			{#if loading && !unitsLoaded}
				<div role="status" aria-label="กำลังโหลดโครงสร้าง"><PageSkeleton variant="detail" /></div>
			{:else if error && !unitsLoaded}
				<PageState
					variant="error"
					title="โหลดโครงสร้างโรงเรียนไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadDepartments}
				/>
			{:else if departments.length === 0}
				<PageState
					title="ยังไม่มีหน่วยงาน"
					description="เพิ่มกลุ่มบริหารหรือฝ่าย/งานเพื่อเริ่มจัดโครงสร้างโรงเรียน"
				/>
			{:else}
				<div class="grid items-start gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
					<div
						class={`min-w-0 space-y-4 ${selectedUnitId ? 'hidden lg:block' : ''}`}
						data-testid="organization-hierarchy"
					>
						<div class="space-y-3 rounded-xl border bg-card p-3 sm:p-4">
							<div class="space-y-2">
								<Label for="organization-search">ค้นหากลุ่มบริหารหรือฝ่าย/งาน</Label>
								<div class="relative">
									<Search
										class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
									/><Input
										id="organization-search"
										value={searchQuery}
										oninput={(event) =>
											filterHierarchy(event.currentTarget.value, activeUnitTypeFilter)}
										placeholder="ชื่อหน่วยงานหรือชื่อภาษาอังกฤษ"
										class="pl-10"
									/>
								</div>
							</div>
							<ToggleGroup.Root
								type="single"
								bind:value={
									() => activeUnitTypeFilter,
									(value) => {
										const filter = unitTypeFilters.find((item) => item.value === value);
										if (filter) filterHierarchy(searchQuery, filter.value);
									}
								}
								aria-label="ประเภทหน่วยงาน"
								class="flex-wrap justify-start"
							>
								{#each unitTypeFilters as filter (filter.value)}<ToggleGroup.Item
										value={filter.value}>{filter.label}</ToggleGroup.Item
									>{/each}
							</ToggleGroup.Root>
						</div>
						{#if visibleTreeRoots.length === 0}
							<PageState
								title="ไม่พบหน่วยงานที่ตรงเงื่อนไข"
								description="ลองปรับคำค้นหาหรือเลือกประเภททั้งหมด"
							/>
						{:else}
							<OrganizationHierarchy
								roots={visibleTreeRoots}
								selectedId={selectedUnit?.id}
								onSelect={handleSelectUnit}
							/>
						{/if}
					</div>
					{#if selectedUnit}
						<div
							class={`min-w-0 scroll-mt-20 space-y-4 ${selectedUnitId ? '' : 'hidden lg:block'}`}
							data-testid="organization-selected-detail"
						>
							<Card.Root class="gap-0 overflow-hidden py-0">
								<div
									class="space-y-4 border-b border-primary/10 bg-gradient-to-br from-primary/5 via-card to-card p-4 sm:p-5"
								>
									<nav
										aria-label="สายสังกัด"
										class="flex flex-wrap items-center gap-x-1 gap-y-2 text-sm text-muted-foreground"
									>
										{#each selectedUnitAncestors as ancestor (ancestor.id)}<Button
												variant="link"
												size="sm"
												class="h-auto max-w-full whitespace-normal p-0 text-left"
												onclick={() => handleSelectUnit(ancestor.id)}>{ancestor.name}</Button
											><ChevronRight class="size-3 shrink-0" />{/each}
										<span class="break-words">{selectedUnit.name}</span>
									</nav>
									<div class="space-y-2">
										<div class="flex flex-wrap gap-2">
											<Badge variant="secondary">{unitTypeLabel(selectedUnit)}</Badge><Badge
												variant="outline"
												class={selectedUnit.is_active
													? 'border-success/20 bg-success/10 text-success'
													: ''}>{selectedUnit.is_active ? 'ใช้งาน' : 'ปิดใช้งาน'}</Badge
											>
										</div>
										<h2
											bind:this={detailHeading}
											tabindex="-1"
											class="scroll-mt-4 break-words text-xl font-semibold outline-none"
										>
											{selectedUnit.name}
										</h2>
										{#if selectedUnit.name_en}<p class="break-words text-sm text-muted-foreground">
												{selectedUnit.name_en}
											</p>{/if}
									</div>
									<div class="flex flex-wrap gap-2">
										<Button
											size="sm"
											href={organizationDetailHref(selectedUnit.id, currentUrl)}
											data-sveltekit-preload-data="tap"
											><ArrowRight class="size-4" />เปิดรายละเอียด</Button
										>
										{#if canUpdateOrganizationUnit}<Button
												size="sm"
												variant="outline"
												onclick={() => handleEdit(selectedUnit)}
												><Pencil class="size-4" />แก้ไข</Button
											>{/if}
										{#if canReadOrganizationPermissions}<Button
												size="sm"
												variant="outline"
												onclick={() => handlePermission(selectedUnit)}
												><KeyRound class="size-4" />{canUpdateOrganizationPermissions
													? 'สิทธิ์'
													: 'ดูสิทธิ์'}</Button
											>{/if}
										{#if canAssignOrganizationMembers}<Button
												size="sm"
												variant="outline"
												href={organizationDetailHref(selectedUnit.id, currentUrl)}
												data-sveltekit-preload-data="tap"><Users class="size-4" />สมาชิก</Button
											>{/if}
									</div>
								</div>
								<Card.Content class="space-y-4 py-4 sm:py-5">
									<dl class="grid gap-4 text-sm sm:grid-cols-2">
										<div>
											<dt class="text-muted-foreground">
												{selectedManagementGroup ? 'สังกัดกลุ่มบริหาร' : 'อยู่ภายใต้'}
											</dt>
											<dd class="mt-1 break-words font-medium">
												{selectedManagementGroup?.name ??
													selectedUnitAncestors.at(-1)?.name ??
													(isSchoolRoot(selectedUnit) ? 'ระดับโรงเรียน' : 'ไม่พบหน่วยงานต้นสังกัด')}
											</dd>
										</div>
										<div>
											<dt class="text-muted-foreground">หมวดงาน</dt>
											<dd class="mt-1">{categoryLabel(selectedUnit)}</dd>
										</div>
										{#if selectedUnit.location}<div>
												<dt class="text-muted-foreground">สถานที่</dt>
												<dd class="mt-1 break-words">{selectedUnit.location}</dd>
											</div>{/if}
										{#if selectedUnit.phone || selectedUnit.email}<div>
												<dt class="text-muted-foreground">ติดต่อ</dt>
												<dd class="mt-1 break-words">
													{[selectedUnit.phone, selectedUnit.email].filter(Boolean).join(' · ')}
												</dd>
											</div>{/if}
									</dl>
									{#if selectedUnit.description}<p
											class="whitespace-pre-line break-words text-sm text-muted-foreground"
										>
											{selectedUnit.description}
										</p>{/if}
									{#if selectedUnit.is_active && canDeleteOrganizationUnit && !selectedUnit.is_system}<Button
											size="sm"
											variant="ghost"
											class="text-destructive"
											onclick={() => (showDeactivateDialog = true)}
											disabled={statusMutationPending}><Power class="size-4" />ปิดใช้งาน</Button
										>
									{:else if !selectedUnit.is_active && canUpdateOrganizationUnit}<Button
											size="sm"
											variant="outline"
											onclick={handleReactivateOrganizationUnit}
											disabled={statusMutationPending}
											><RotateCcw class="size-4" />{statusMutationPending
												? 'กำลังเปิดใช้งาน...'
												: 'เปิดใช้งาน'}</Button
										>{/if}
									{#if canDeleteOrganizationUnit && selectedUnit.unit_type === 'management_group' && !selectedUnit.is_system && !selectedUnit.subject_group_id}
										<Button
											size="sm"
											variant="destructive"
											disabled={statusMutationPending || permanentDeletePending}
											onclick={() => {
												deletingUnit = selectedUnit;
												permanentDeleteError = '';
											}}><Trash2 class="size-4" />ลบกลุ่มบริหารถาวร</Button
										>
									{/if}
								</Card.Content>
							</Card.Root>
							{#if selectedUnitChildren.length || canCreateOrganizationUnit}
								<Card.Root class="gap-0 py-0">
									<div class="flex flex-wrap items-center justify-between gap-3 border-b px-4 py-3">
										<h3 class="font-semibold">
											{selectedUnit.unit_type === 'management_group'
												? 'ฝ่าย/งานในกลุ่มนี้'
												: 'หน่วยงานภายใต้'}
											<span class="font-normal text-muted-foreground"
												>({selectedUnitChildren.length})</span
											>
										</h3>
										{#if canCreateOrganizationUnit}<Button
												variant="outline"
												size="sm"
												onclick={() => handleCreate(selectedUnit)}
												><Plus class="size-4" />เพิ่มฝ่าย/งาน</Button
											>{/if}
									</div>
									<Card.Content class="py-3">
										{#if selectedUnitChildren.length}<ul class="divide-y">
												{#each selectedUnitChildren as child (child.id)}<li>
														<Button
															variant="ghost"
															class="h-auto w-full justify-start whitespace-normal py-2 text-left"
															onclick={() => handleSelectUnit(child.id)}
															><Building2 class="size-4 text-muted-foreground" /><span
																class="min-w-0 flex-1 break-words">{child.name}</span
															>{#if !child.is_active}<Badge variant="outline">ปิดใช้งาน</Badge
																>{/if}<ChevronRight class="size-4" /></Button
														>
													</li>{/each}
											</ul>
										{:else}<p class="text-sm text-muted-foreground">
												ยังไม่มีฝ่าย/งานภายใต้หน่วยงานนี้
											</p>{/if}
									</Card.Content>
								</Card.Root>
							{/if}
							<Card.Root class="gap-0 py-0">
								<div class="flex items-center justify-between gap-3 border-b px-4 py-3">
									<h3 class="flex items-center gap-2 font-semibold">
										<Users class="size-4" />หัวหน้าและสมาชิก
									</h3>
									<span class="text-sm text-muted-foreground"
										>{membersLoaded ? selectedMembers.length : '—'} คน</span
									>
								</div>
								<Card.Content class="py-4">
									<section
										data-testid="organization-selected-members"
										aria-busy={selectedMembersLoading}
									>
										{#if selectedMembersError && membersLoaded}<PageState
												variant="error"
												title="โหลดสมาชิกไม่สำเร็จ"
												description={selectedMembersError}
												actionLabel="ลองอีกครั้ง"
												onaction={() => selectedUnit && loadSelectedMembers(selectedUnit.id)}
											/>{/if}
										{#if selectedMembersLoading && !membersLoaded}<div
												role="status"
												aria-label="กำลังโหลดสมาชิก"
											>
												<PageSkeleton variant="table" rows={3} />
											</div>
										{:else if selectedMembersError && !membersLoaded}<PageState
												variant="error"
												title="โหลดสมาชิกไม่สำเร็จ"
												description={selectedMembersError}
												actionLabel="ลองอีกครั้ง"
												onaction={() => selectedUnit && loadSelectedMembers(selectedUnit.id)}
											/>
										{:else if selectedMembers.length === 0}
											<div
												class="rounded-md border border-dashed py-6 text-center text-sm text-muted-foreground"
											>
												ยังไม่มีสมาชิกในหน่วยงานนี้
											</div>
										{:else}
											<div class="divide-y rounded-md border">
												{#each selectedMembers.slice(0, 5) as member (member.user_id + '-' + member.organization_unit_id)}
													<div class="flex items-center justify-between gap-3 px-3 py-2">
														<div class="min-w-0">
															<p class="break-words text-sm font-medium">{member.name}</p>
															<p class="text-xs text-muted-foreground">
																{member.position_title || positionLabel(member.position_code)}
																{#if !member.isDirect || member.sourceAssignments.length > 1}<span
																		class="block break-words"
																		>จาก {member.sourceAssignments
																			.map((source) => source.organization_unit_name)
																			.join(' · ')}</span
																	>{/if}
															</p>
														</div>
														{#if member.is_primary}
															<Badge variant="outline" class="shrink-0 text-[10px]">หลัก</Badge>
														{/if}
													</div>
												{/each}
											</div>
											{#if selectedMembers.length > 5}
												<p class="text-xs text-muted-foreground">
													และอีก {selectedMembers.length - 5} คนในหน้ารายละเอียด
												</p>
											{/if}
										{/if}
									</section>
								</Card.Content>
							</Card.Root>
						</div>
					{/if}
				</div>
			{/if}
		</section>
		{#if showDialog && (canCreateOrganizationUnit || canUpdateOrganizationUnit)}<OrganizationUnitDialog
				bind:open={showDialog}
				organizationUnitToEdit={editingDepartment}
				organizationUnits={departments}
				onSuccess={loadDepartments}
				forcedCategory="general"
				forcedParentId={editingDepartment ? undefined : createParentId}
			/>{/if}
		{#if showPermissionDialog && canReadOrganizationPermissions}<OrganizationPermissionDialog
				bind:open={showPermissionDialog}
				organizationUnit={permissionDepartment}
				readOnly={!canUpdateOrganizationPermissions}
			/>{/if}
	{/if}
</PageShell>

<Dialog bind:open={showDeactivateDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>ยืนยันการปิดใช้งานหน่วยงาน</DialogTitle>
			<DialogDescription>
				สิทธิ์ที่ผู้ใช้ได้รับจากหน่วยงาน <strong>{selectedUnit?.name}</strong> จะหยุดมีผลทันที ประวัติ
				สมาชิก การกำหนดสิทธิ์ และการมอบหมายเดิมจะยังคงอยู่ และสามารถเปิดใช้งานกลับได้ภายหลัง
			</DialogDescription>
		</DialogHeader>
		<DialogFooter>
			<Button
				variant="outline"
				onclick={() => (showDeactivateDialog = false)}
				disabled={statusMutationPending}
			>
				ยกเลิก
			</Button>
			<Button
				variant="destructive"
				onclick={handleDeactivateOrganizationUnit}
				disabled={statusMutationPending}
				class="gap-2"
			>
				<Power class="h-4 w-4" />
				{statusMutationPending ? 'กำลังปิดใช้งาน...' : 'ปิดใช้งานหน่วยงาน'}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<AlertDialog.Root
	open={deletingUnit !== null}
	onOpenChange={(open) => {
		if (!open && !permanentDeletePending) deletingUnit = null;
	}}
>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ลบกลุ่มบริหารถาวร</AlertDialog.Title>
			<AlertDialog.Description
				>ลบ “{deletingUnit?.name}” ออกจากระบบ การดำเนินการนี้ย้อนกลับไม่ได้
				ลบได้เฉพาะกลุ่มที่ไม่มีหน่วยงานย่อย สมาชิก ประวัติ หรือข้อมูลอ้างอิง
				หากยังมีข้อมูลให้ใช้การปิดใช้งานแทน</AlertDialog.Description
			>
		</AlertDialog.Header>
		{#if permanentDeleteError}<PageState
				variant="error"
				title="ลบกลุ่มบริหารไม่สำเร็จ"
				description={permanentDeleteError}
			/>{/if}
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={permanentDeletePending}>ยกเลิก</AlertDialog.Cancel>
			<Button
				variant="destructive"
				disabled={permanentDeletePending}
				onclick={handlePermanentDelete}
				>{permanentDeletePending ? 'กำลังลบ...' : 'ยืนยันลบถาวร'}</Button
			>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
