<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { requireApiData } from '#lib/api/client.js';
	import { resolve } from '$app/paths';
	import {
		deleteOrganizationUnit,
		listOrganizationMembers,
		listOrganizationUnits,
		updateOrganizationUnit
	} from '#lib/api/staff.js';
	import type { OrganizationMemberItem, OrganizationUnit } from '#lib/api/staff.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
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
	import MobileDragDropPolyfill from '#lib/components/MobileDragDropPolyfill.svelte';
	import {
		ArrowRight,
		Building2,
		Briefcase,
		GraduationCap,
		KeyRound,
		Network,
		Pencil,
		Plus,
		Power,
		RotateCcw,
		Search,
		School,
		Users
	} from '@lucide/svelte';
	import OrganizationUnitDialog from '#lib/components/staff/OrganizationUnitDialog.svelte';
	import OrganizationPermissionDialog from '#lib/components/staff/OrganizationPermissionDialog.svelte';
	import { toast } from 'svelte-sonner';

	type UnitTypeFilter = 'all' | 'management_group' | 'subject_group' | 'division' | 'other';

	const unitTypeFilters: { value: UnitTypeFilter; label: string }[] = [
		{ value: 'all', label: 'ทั้งหมด' },
		{ value: 'management_group', label: 'กลุ่มบริหาร' },
		{ value: 'subject_group', label: 'กลุ่มสาระ' },
		{ value: 'division', label: 'ฝ่าย/งาน' },
		{ value: 'other', label: 'อื่น ๆ' }
	];

	let departments: OrganizationUnit[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	let searchQuery = $state('');
	let activeUnitTypeFilter = $state<UnitTypeFilter>('all');
	let selectedUnitId = $state<string | null>(null);

	let draggedDeptId: string | null = $state(null);
	let dragOverDeptId: string | null = $state(null);

	let showDialog = $state(false);
	let editingDepartment: OrganizationUnit | null = $state(null);
	let showPermissionDialog = $state(false);
	let permissionDepartment = $state<OrganizationUnit | null>(null);
	let showDeactivateDialog = $state(false);
	let statusMutationPending = $state(false);

	let selectedMembers = $state<OrganizationMemberItem[]>([]);
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
	const canCreateOrganizationUnit = $derived($can.has(PERMISSIONS.ROLES_CREATE_ALL));
	const canUpdateOrganizationUnit = $derived($can.has(PERMISSIONS.ROLES_UPDATE_ALL));
	const canDeleteOrganizationUnit = $derived($can.has(PERMISSIONS.ROLES_DELETE_ALL));
	const canReadOrganizationPermissions = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const canUpdateOrganizationPermissions = $derived($can.has(PERMISSIONS.ROLES_UPDATE_ALL));
	const canAssignOrganizationMembers = $derived($can.has(PERMISSIONS.ROLES_ASSIGN_ALL));

	let organizationStats = $derived.by(() => ({
		total: departments.length,
		managementGroups: departments.filter((unit) => unit.unit_type === 'management_group').length,
		subjectGroups: departments.filter((unit) => unit.unit_type === 'subject_group').length,
		divisions: departments.filter((unit) => unit.unit_type === 'division').length,
		active: departments.filter((unit) => unit.is_active).length
	}));

	let visibleUnits = $derived.by(() => {
		const query = searchQuery.trim().toLowerCase();

		return departments
			.filter((unit) => {
				const matchesSearch =
					!query ||
					unit.name.toLowerCase().includes(query) ||
					unit.code.toLowerCase().includes(query) ||
					(unit.name_en?.toLowerCase().includes(query) ?? false);

				if (!matchesSearch) return false;
				if (activeUnitTypeFilter === 'all') return true;
				if (activeUnitTypeFilter === 'other') {
					return !['management_group', 'subject_group', 'division', 'school'].includes(
						unit.unit_type ?? ''
					);
				}

				return unit.unit_type === activeUnitTypeFilter;
			})
			.sort((a, b) => (a.display_order || 0) - (b.display_order || 0));
	});

	let visibleTreeRoots = $derived.by(() => {
		const visibleIds = new Set(visibleUnits.map((unit) => unit.id));
		return visibleUnits.filter(
			(unit) => !unit.parent_unit_id || !visibleIds.has(unit.parent_unit_id)
		);
	});

	let selectedUnit = $derived.by(() => {
		return (
			departments.find((unit) => unit.id === selectedUnitId) ??
			visibleUnits[0] ??
			departments.find((unit) => isSchoolRoot(unit)) ??
			null
		);
	});

	let selectedUnitChildren = $derived.by(() =>
		selectedUnit ? getChildren(selectedUnit.id, departments) : []
	);

	let selectedUnitParent = $derived.by(() =>
		selectedUnit?.parent_unit_id
			? departments.find((unit) => unit.id === selectedUnit?.parent_unit_id)
			: null
	);

	function getChildren(parentId: string, source = visibleUnits): OrganizationUnit[] {
		return source
			.filter((unit) => unit.parent_unit_id === parentId)
			.sort((a, b) => (a.display_order || 0) - (b.display_order || 0));
	}

	function isSchoolRoot(unit: OrganizationUnit): boolean {
		return unit.code === 'SCHOOL' || unit.unit_type === 'school';
	}

	function unitTypeLabel(unit: OrganizationUnit): string {
		if (isSchoolRoot(unit)) return 'โรงเรียน';
		if (unit.unit_type === 'management_group') return 'กลุ่มบริหาร';
		if (unit.unit_type === 'subject_group') return 'กลุ่มสาระ';
		if (unit.unit_type === 'division') return 'ฝ่าย/งาน';
		if (unit.unit_type === 'committee') return 'คณะกรรมการ';
		if (unit.unit_type === 'team') return 'ทีม';
		return 'หน่วยงาน';
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

	function isDescendantOf(unitId: string, possibleAncestorId: string): boolean {
		let current = departments.find((unit) => unit.id === unitId);
		while (current?.parent_unit_id) {
			if (current.parent_unit_id === possibleAncestorId) return true;
			current = departments.find((unit) => unit.id === current?.parent_unit_id);
		}
		return false;
	}

	function applyUnits(result: Awaited<typeof data.units>, revision: number) {
		if (!unitsRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			departments = result.data;
			unitsLoaded = true;
			if (!selectedUnitId)
				selectedUnitId = (result.data.find(isSchoolRoot) ?? result.data[0])?.id ?? null;
		} else error = result.error;
	}
	async function loadDepartments() {
		if (!canReadOrganization) return;
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
		if (!canReadOrganization || !unitId) return;
		const ticket = membersRequest.begin();
		if (selectedMembersUnitId !== unitId) {
			selectedMembers = [];
			membersLoaded = false;
		}
		selectedMembersUnitId = unitId;
		selectedMembersLoading = true;
		selectedMembersError = '';
		const result = await captureRouteLoad(
			listOrganizationMembers(unitId, undefined, { signal: ticket.signal }).then((reply) =>
				requireApiData(reply, 'โหลดสมาชิกไม่สำเร็จ')
			),
			'โหลดสมาชิกไม่สำเร็จ'
		);
		if (!membersRequest.isCurrent(ticket.revision) || selectedUnit?.id !== unitId) return;
		selectedMembersLoading = false;
		if (result.ok) {
			selectedMembers = result.data;
			membersLoaded = true;
		} else selectedMembersError = result.error;
	}

	function handleCreate() {
		if (!canCreateOrganizationUnit) return;
		editingDepartment = null;
		showDialog = true;
	}

	function handleEdit(unit: OrganizationUnit) {
		if (!canUpdateOrganizationUnit) return;
		editingDepartment = unit;
		showDialog = true;
	}

	function handleSelectUnit(unitId: string) {
		selectedUnitId = unitId;
	}

	function handlePermission(unit: OrganizationUnit) {
		if (!canReadOrganizationPermissions) return;
		permissionDepartment = unit;
		showPermissionDialog = true;
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

	function handleDragStart(event: DragEvent, unitId: string) {
		event.stopPropagation();
		if (!canUpdateOrganizationUnit) return;
		if (!event.dataTransfer) return;
		draggedDeptId = unitId;
		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData('text/plain', unitId);
	}

	function handleDragEnd() {
		draggedDeptId = null;
		dragOverDeptId = null;
	}

	function handleDragOver(event: DragEvent, unitId: string) {
		if (!canUpdateOrganizationUnit) return;
		event.preventDefault();
		event.stopPropagation();
		if (draggedDeptId === unitId) return;
		dragOverDeptId = unitId;
	}

	async function handleDrop(event: DragEvent, targetParentId: string | null) {
		event.preventDefault();
		event.stopPropagation();
		dragOverDeptId = null;
		if (!canUpdateOrganizationUnit) return;
		const sourceDeptId = event.dataTransfer?.getData('text/plain');

		if (!sourceDeptId || sourceDeptId === targetParentId) return;

		const sourceDept = departments.find((unit) => unit.id === sourceDeptId);
		if (!sourceDept) return;

		if (isSchoolRoot(sourceDept)) {
			toast.error('ไม่สามารถย้ายหน่วยรากของโรงเรียนได้');
			return;
		}

		if (targetParentId && isDescendantOf(targetParentId, sourceDept.id)) {
			toast.error('ไม่สามารถย้ายหน่วยงานไปอยู่ใต้หน่วยงานย่อยของตัวเองได้');
			return;
		}

		if (statusMutationPending) return;
		unitsRequest.abort();
		loading = false;
		statusMutationPending = true;
		const loadingToast = toast.loading('กำลังย้ายหน่วยงาน...');
		try {
			const result = await updateOrganizationUnit(sourceDeptId, {
				parent_unit_id: targetParentId ?? undefined
			});

			if (disposed) {
				toast.dismiss(loadingToast);
				return;
			}
			if (result.success) {
				toast.success('ย้ายหน่วยงานสำเร็จ', { id: loadingToast });
				await loadDepartments();
			} else {
				toast.error('ย้ายหน่วยงานไม่สำเร็จ: ' + result.error, { id: loadingToast });
			}
		} catch (err: unknown) {
			if (disposed) {
				toast.dismiss(loadingToast);
				return;
			}
			toast.error('เกิดข้อผิดพลาด: ' + (err instanceof Error ? err.message : String(err)), {
				id: loadingToast
			});
		} finally {
			if (!disposed) statusMutationPending = false;
		}
	}

	$effect.pre(() => {
		const source = unitsSource,
			memberRead = membersSource;
		untrack(() => {
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
					? ((result.data.find(isSchoolRoot) ?? result.data[0])?.id ?? '')
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
					selectedMembers = result.data.members;
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
			allowed = canReadOrganization;
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

<MobileDragDropPolyfill />

<PageShell
	title="โครงสร้างโรงเรียน"
	description="แผนที่หน่วยงาน กลุ่มบริหาร กลุ่มสาระ และตำแหน่งที่ใช้เป็นฐานสิทธิ์"
>
	{#snippet actions()}
		<Button variant="outline" onclick={loadDepartments} disabled={loading || !canReadOrganization}
			>รีเฟรชโครงสร้าง</Button
		>
		{#if canCreateOrganizationUnit}
			<Button onclick={handleCreate} class="gap-2 self-start">
				<Plus class="h-4 w-4" />
				เพิ่มหน่วยงาน
			</Button>
		{/if}
	{/snippet}

	<section data-testid="organization-catalog" aria-busy={loading}>
		{#if loading && unitsLoaded}<p role="status">กำลังอัปเดตโครงสร้าง...</p>{/if}
		{#if error && unitsLoaded}<PageState
				title="อัปเดตโครงสร้างไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadDepartments}
			/>{/if}
		<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
			<div class="rounded-lg border bg-card p-4">
				<div class="flex items-center justify-between">
					<p class="text-sm text-muted-foreground">หน่วยงานทั้งหมด</p>
					<Network class="h-4 w-4 text-muted-foreground" />
				</div>
				<p class="mt-2 text-2xl font-semibold">{unitsLoaded ? organizationStats.total : '—'}</p>
			</div>
			<div class="rounded-lg border bg-card p-4">
				<div class="flex items-center justify-between">
					<p class="text-sm text-muted-foreground">กลุ่มบริหาร</p>
					<Briefcase class="h-4 w-4 text-muted-foreground" />
				</div>
				<p class="mt-2 text-2xl font-semibold">
					{unitsLoaded ? organizationStats.managementGroups : '—'}
				</p>
			</div>
			<div class="rounded-lg border bg-card p-4">
				<div class="flex items-center justify-between">
					<p class="text-sm text-muted-foreground">กลุ่มสาระ</p>
					<GraduationCap class="h-4 w-4 text-muted-foreground" />
				</div>
				<p class="mt-2 text-2xl font-semibold">
					{unitsLoaded ? organizationStats.subjectGroups : '—'}
				</p>
			</div>
			<div class="rounded-lg border bg-card p-4">
				<div class="flex items-center justify-between">
					<p class="text-sm text-muted-foreground">ฝ่าย/งาน</p>
					<Building2 class="h-4 w-4 text-muted-foreground" />
				</div>
				<p class="mt-2 text-2xl font-semibold">{unitsLoaded ? organizationStats.divisions : '—'}</p>
			</div>
			<div class="rounded-lg border bg-card p-4">
				<div class="flex items-center justify-between">
					<p class="text-sm text-muted-foreground">ใช้งานอยู่</p>
					<School class="h-4 w-4 text-muted-foreground" />
				</div>
				<p class="mt-2 text-2xl font-semibold">{unitsLoaded ? organizationStats.active : '—'}</p>
			</div>
		</div>

		<div class="rounded-xl border bg-card p-3 sm:p-4">
			<div class="flex flex-col gap-3 xl:flex-row xl:items-center">
				<div class="relative flex-1">
					<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
					<Input
						type="text"
						bind:value={searchQuery}
						placeholder="ค้นหาหน่วยงาน รหัส หรือชื่อภาษาอังกฤษ"
						class="pl-10"
					/>
				</div>
				<div class="flex gap-2 overflow-x-auto">
					{#each unitTypeFilters as filter (filter.value)}
						<button
							type="button"
							class="shrink-0 rounded-md border px-3 py-2 text-sm transition-colors {activeUnitTypeFilter ===
							filter.value
								? 'border-primary bg-primary text-primary-foreground'
								: 'bg-background text-muted-foreground hover:bg-muted hover:text-foreground'}"
							onclick={() => (activeUnitTypeFilter = filter.value)}
						>
							{filter.label}
						</button>
					{/each}
				</div>
			</div>
		</div>

		{#if !canReadOrganization}
			<PageState
				variant="permission"
				title="ไม่มีสิทธิ์ดูโครงสร้างโรงเรียน"
				description="บัญชีนี้เข้า module บทบาทได้ แต่ยังไม่มีสิทธิ์อ่านข้อมูลโครงสร้างโรงเรียน"
			/>
		{:else if loading && !unitsLoaded}
			<div role="status" aria-label="กำลังโหลดโครงสร้าง"><PageSkeleton variant="detail" /></div>
		{:else if error && !unitsLoaded}
			<PageState
				variant="error"
				title="โหลดโครงสร้างโรงเรียนไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadDepartments}
			/>
		{:else}
			<div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_420px]">
				<section class="min-h-[520px] rounded-lg border bg-card">
					<div class="flex items-center justify-between border-b px-4 py-3">
						<div>
							<h2 class="text-sm font-semibold">แผนที่องค์กร</h2>
							<p class="text-xs text-muted-foreground">
								{visibleUnits.length} หน่วยงานที่ตรงเงื่อนไข
							</p>
						</div>
						{#if draggedDeptId && canUpdateOrganizationUnit}
							<div
								class="rounded-md border border-dashed px-3 py-1.5 text-xs text-muted-foreground"
								ondragover={(event) => {
									event.preventDefault();
									dragOverDeptId = 'root';
								}}
								ondrop={(event) => handleDrop(event, null)}
								role="button"
								tabindex="0"
							>
								ย้ายขึ้นระดับบน
							</div>
						{/if}
					</div>

					{#if visibleTreeRoots.length === 0}
						<PageState
							title="ไม่พบหน่วยงานที่ตรงเงื่อนไข"
							description="ลองปรับคำค้นหาหรือตัวกรองหมวดงาน"
							class="border-0 shadow-none"
						/>
					{:else}
						<div class="space-y-1 p-3">
							{#snippet unitTreeNode(unit: OrganizationUnit, depth: number)}
								{@const children = getChildren(unit.id)}
								<div style={`padding-left: ${Math.min(depth, 5) * 14}px`}>
									<button
										type="button"
										class="group flex w-full items-center gap-3 rounded-md border px-3 py-2.5 text-left transition-colors {selectedUnit?.id ===
										unit.id
											? 'border-primary bg-primary/10'
											: 'border-transparent hover:border-border hover:bg-muted/40'} {dragOverDeptId ===
										unit.id
											? 'ring-2 ring-primary'
											: ''}"
										draggable={canUpdateOrganizationUnit && !isSchoolRoot(unit)}
										ondragstart={(event) => handleDragStart(event, unit.id)}
										ondragend={handleDragEnd}
										ondragover={(event) => handleDragOver(event, unit.id)}
										ondrop={(event) => handleDrop(event, unit.id)}
										onclick={() => handleSelectUnit(unit.id)}
									>
										<span
											class="h-9 w-1 shrink-0 rounded-full {unit.is_active
												? 'bg-emerald-500'
												: 'bg-muted-foreground/30'}"
										></span>
										<span class="min-w-0 flex-1">
											<span class="flex min-w-0 items-center gap-2">
												<span class="truncate text-sm font-medium">{unit.name}</span>
												<Badge variant="secondary" class="shrink-0 text-[10px]">
													{unitTypeLabel(unit)}
												</Badge>
												{#if unit.is_system}
													<Badge variant="outline" class="shrink-0 text-[10px]">ระบบ</Badge>
												{/if}
											</span>
											<span class="mt-0.5 flex items-center gap-2 text-xs text-muted-foreground">
												<span class="font-mono">{unit.code}</span>
												{#if children.length > 0}
													<span>{children.length} หน่วยงานย่อย</span>
												{/if}
											</span>
										</span>
										<ArrowRight
											class="h-4 w-4 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
										/>
									</button>
								</div>

								{#if children.length > 0}
									<div class="mt-1 space-y-1">
										{#each children as child (child.id)}
											{@render unitTreeNode(child, depth + 1)}
										{/each}
									</div>
								{/if}
							{/snippet}

							{#each visibleTreeRoots as root (root.id)}
								{@render unitTreeNode(root, 0)}
							{/each}
						</div>
					{/if}
				</section>

				<aside class="rounded-lg border bg-card">
					{#if selectedUnit}
						<div class="border-b p-5">
							<div class="flex items-start justify-between gap-4">
								<div class="min-w-0">
									<div class="flex flex-wrap items-center gap-2">
										<Badge variant="outline">{selectedUnit.code}</Badge>
										<Badge variant="secondary">{unitTypeLabel(selectedUnit)}</Badge>
										<Badge variant={selectedUnit.is_active ? 'default' : 'outline'}>
											{selectedUnit.is_active ? 'ใช้งาน' : 'ปิดใช้งาน'}
										</Badge>
										{#if selectedUnit.is_system}
											<Badge variant="outline">หน่วยงานระบบ</Badge>
										{/if}
									</div>
									<h2 class="mt-3 text-xl font-semibold leading-tight">{selectedUnit.name}</h2>
									{#if selectedUnit.name_en}
										<p class="mt-1 text-sm text-muted-foreground">{selectedUnit.name_en}</p>
									{/if}
								</div>
							</div>
							<p class="mt-4 text-sm text-muted-foreground">
								{selectedUnit.description || 'ยังไม่มีรายละเอียด'}
							</p>
						</div>

						<div class="space-y-5 p-5">
							<div class="grid grid-cols-2 gap-3">
								<div class="rounded-md border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">หมวดงาน</p>
									<p class="mt-1 text-sm font-medium">{categoryLabel(selectedUnit)}</p>
								</div>
								<div class="rounded-md border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">หน่วยงานย่อย</p>
									<p class="mt-1 text-sm font-medium">{selectedUnitChildren.length}</p>
								</div>
							</div>

							<div class="space-y-2 text-sm">
								<div class="flex items-center justify-between gap-3">
									<span class="text-muted-foreground">อยู่ภายใต้</span>
									<span class="truncate font-medium"
										>{selectedUnitParent?.name ?? 'ระดับบนสุด'}</span
									>
								</div>
								<div class="flex items-center justify-between gap-3">
									<span class="text-muted-foreground">สถานที่</span>
									<span class="truncate font-medium">{selectedUnit.location || '-'}</span>
								</div>
								<div class="flex items-center justify-between gap-3">
									<span class="text-muted-foreground">ติดต่อ</span>
									<span class="truncate font-medium"
										>{selectedUnit.phone || selectedUnit.email || '-'}</span
									>
								</div>
							</div>

							<div class="space-y-3">
								<div class="flex items-center justify-between">
									<h3 class="flex items-center gap-2 text-sm font-semibold">
										<Users class="h-4 w-4" />
										สมาชิก
									</h3>
									<span class="text-xs text-muted-foreground"
										>{membersLoaded ? selectedMembers.length : '—'} คน</span
									>
								</div>

								<section
									data-testid="organization-selected-members"
									aria-busy={selectedMembersLoading}
								>
									{#if selectedMembersLoading && membersLoaded}<p role="status">
											กำลังอัปเดตสมาชิก...
										</p>{/if}
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
														<p class="truncate text-sm font-medium">{member.name}</p>
														<p class="text-xs text-muted-foreground">
															{member.position_title || positionLabel(member.position_code)}
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
							</div>

							<div class="grid gap-2">
								<Button
									href={resolve(`staff/organization/${selectedUnit.id}`)}
									data-sveltekit-preload-data="tap"
									class="gap-2"
								>
									<ArrowRight class="h-4 w-4" />
									เปิดรายละเอียด
								</Button>
								{#if selectedUnit.is_active && canDeleteOrganizationUnit && !selectedUnit.is_system}
									<Button
										variant="destructive"
										onclick={() => (showDeactivateDialog = true)}
										disabled={statusMutationPending}
										class="gap-2"
									>
										<Power class="h-4 w-4" />
										ปิดใช้งาน
									</Button>
								{:else if !selectedUnit.is_active && canUpdateOrganizationUnit}
									<Button
										variant="outline"
										onclick={handleReactivateOrganizationUnit}
										disabled={statusMutationPending}
										class="gap-2"
									>
										<RotateCcw class="h-4 w-4" />
										{statusMutationPending ? 'กำลังเปิดใช้งาน...' : 'เปิดใช้งาน'}
									</Button>
								{/if}
								{#if canUpdateOrganizationUnit || canReadOrganizationPermissions || canAssignOrganizationMembers}
									<div class="grid grid-cols-2 gap-2">
										{#if canUpdateOrganizationUnit}
											<Button
												variant="outline"
												onclick={() => handleEdit(selectedUnit)}
												class="gap-2"
											>
												<Pencil class="h-4 w-4" />
												แก้ไข
											</Button>
										{/if}
										{#if canReadOrganizationPermissions}
											<Button
												variant="outline"
												onclick={() => handlePermission(selectedUnit)}
												class="gap-2"
											>
												<KeyRound class="h-4 w-4" />
												{canUpdateOrganizationPermissions ? 'สิทธิ์' : 'ดูสิทธิ์'}
											</Button>
										{/if}
										{#if canAssignOrganizationMembers}
											<Button
												variant="outline"
												href={resolve(`staff/organization/${selectedUnit.id}`)}
												data-sveltekit-preload-data="tap"
												class="gap-2"
											>
												<Users class="h-4 w-4" />
												สมาชิก
											</Button>
										{/if}
									</div>
								{/if}
							</div>
						</div>
					{:else}
						<div class="p-12 text-center text-sm text-muted-foreground">
							เลือกหน่วยงานเพื่อดูรายละเอียด
						</div>
					{/if}
				</aside>
			</div>
		{/if}
	</section>
	{#if showDialog && (canCreateOrganizationUnit || canUpdateOrganizationUnit)}
		<OrganizationUnitDialog
			bind:open={showDialog}
			organizationUnitToEdit={editingDepartment}
			organizationUnits={departments}
			onSuccess={loadDepartments}
			forcedCategory="general"
		/>
	{/if}

	{#if showPermissionDialog && canReadOrganizationPermissions}
		<OrganizationPermissionDialog
			bind:open={showPermissionDialog}
			organizationUnit={permissionDepartment}
			readOnly={!canUpdateOrganizationPermissions}
		/>
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
