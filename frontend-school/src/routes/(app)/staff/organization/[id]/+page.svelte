<script lang="ts">
	import type { PageProps } from './$types';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		getOrganizationUnit,
		listOrganizationUnits,
		listDelegations,
		listDelegatablePermissions,
		createDelegation,
		revokeDelegation,
		listOrganizationMembers,
		type OrganizationUnit,
		type DelegationItem,
		type DelegatablePermission,
		type OrganizationMemberItem,
		type CreateDelegationBody
	} from '$lib/api/staff';
	import OrganizationUnitDialog from '$lib/components/staff/OrganizationUnitDialog.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import OrganizationMembersSection from '$lib/components/staff/OrganizationMembersSection.svelte';
	import {
		ArrowLeft,
		ArrowRight,
		Phone,
		Mail,
		MapPin,
		Briefcase,
		GraduationCap,
		Shield,
		Plus,
		Trash2,
		KeyRound,
		Network,
		Info,
		Pencil,
		Users
	} from 'lucide-svelte';
	import OrganizationPermissionDialog from '$lib/components/staff/OrganizationPermissionDialog.svelte';

	const { params }: PageProps = $props();
	type DetailTab = 'members' | 'permissions' | 'children' | 'delegations';

	let deptId = $derived(params.id);
	let department: OrganizationUnit | null = $state(null);
	let allDepartments: OrganizationUnit[] = $state([]);
	let childDepts: OrganizationUnit[] = $state([]);
	let deptMembers: OrganizationMemberItem[] = $state([]);
	let delegations: DelegationItem[] = $state([]);
	let activeTab = $state<DetailTab>('members');
	let showEditDialog = $state(false);
	let showPermissionDialog = $state(false);

	let showAddChildDialog = $state(false);
	let delegatablePerms: DelegatablePermission[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	let showDelegateDialog = $state(false);
	let delegateForm = $state({ to_user_id: '', permission_id: '', reason: '', expires_at: '' });
	let delegateSubmitting = $state(false);
	let delegateError = $state('');

	let canReadOrganization = $derived.by(() => $can.has(PERMISSIONS.ROLES_READ_ALL));
	let canCreateOrganizationUnit = $derived.by(() => $can.has(PERMISSIONS.ROLES_CREATE_ALL));
	let canUpdateOrganizationUnit = $derived.by(() => $can.has(PERMISSIONS.ROLES_UPDATE_ALL));
	let canReadOrganizationPermissions = $derived.by(() => $can.has(PERMISSIONS.ROLES_READ_ALL));
	let canUpdateOrganizationPermissions = $derived.by(() => $can.has(PERMISSIONS.ROLES_UPDATE_ALL));
	let canAssignOrganizationMembers = $derived.by(() => $can.has(PERMISSIONS.ROLES_ASSIGN_ALL));

	let leaderCount = $derived(
		deptMembers.filter((member) =>
			['director', 'deputy_director', 'head'].includes(member.position_code)
		).length
	);

	let primaryMembers = $derived(
		deptMembers.filter((member) =>
			['director', 'deputy_director', 'head', 'deputy_head', 'coordinator'].includes(
				member.position_code
			)
		)
	);

	let canManageDelegations = $derived.by(() =>
		$can.hasAny(
			PERMISSIONS.ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT,
			PERMISSIONS.ROLES_ASSIGN_ALL,
			PERMISSIONS.ROLES_UPDATE_ALL
		)
	);

	let parentUnit = $derived.by(() => {
		const currentDepartment = department;
		if (!currentDepartment?.parent_unit_id) return null;
		return allDepartments.find((unit) => unit.id === currentDepartment.parent_unit_id) ?? null;
	});

	let contextPanel = $derived.by(() => ({
		parentName:
			parentUnit?.name ?? (department?.unit_type === 'school' ? 'ระดับโรงเรียน' : 'ไม่มีข้อมูล'),
		parentCode: parentUnit?.code ?? '',
		childCount: childDepts.length,
		memberCount: deptMembers.length
	}));

	let detailStats = $derived.by(() => {
		const stats = [
			{ label: 'สมาชิก', value: deptMembers.length, helper: 'คนในหน่วยงานนี้' },
			{ label: 'งานหลัก', value: primaryMembers.length, helper: 'หัวหน้าและผู้รับผิดชอบ' },
			{ label: 'หน่วยงานย่อย', value: childDepts.length, helper: 'ใต้โครงสร้างนี้' }
		];

		if (canManageDelegations) {
			stats.push({ label: 'มอบหมายสิทธิ์', value: delegations.length, helper: 'รายการที่ใช้งาน' });
		}

		return stats;
	});

	let contactItems = $derived.by(() => [
		{ key: 'phone', label: 'โทรศัพท์', value: department?.phone || '-' },
		{ key: 'email', label: 'อีเมล', value: department?.email || '-' },
		{ key: 'location', label: 'สถานที่', value: department?.location || '-' }
	]);

	let unitTypeText = $derived.by(() => {
		if (!department) return '-';
		if (department.unit_type === 'school') return 'โรงเรียน';
		if (department.unit_type === 'management_group') return 'กลุ่มบริหาร';
		if (department.unit_type === 'subject_group') return 'กลุ่มสาระ';
		if (department.unit_type === 'division') return 'ฝ่าย/งาน';
		if (department.unit_type === 'committee') return 'คณะกรรมการ';
		if (department.unit_type === 'team') return 'ทีม';
		return 'หน่วยงาน';
	});

	let categoryText = $derived.by(() => {
		if (!department) return '-';
		if (department.category === 'academic') return 'วิชาการ';
		if (department.category === 'student_affairs') return 'กิจการนักเรียน';
		if (department.category === 'personnel') return 'บุคคล';
		if (department.category === 'budget') return 'งบประมาณ';
		if (department.category === 'general') return 'ทั่วไป';
		if (department.category === 'administrative') return 'บริหาร';
		return 'อื่น ๆ';
	});

	let detailTabs = $derived.by(() => {
		const tabs: { id: DetailTab; label: string; count?: number }[] = [
			{ id: 'members', label: 'สมาชิก', count: deptMembers.length },
			{ id: 'permissions', label: 'สิทธิ์ตามตำแหน่ง' },
			{ id: 'children', label: 'หน่วยงานย่อย', count: childDepts.length }
		];

		if (canManageDelegations) {
			tabs.push({ id: 'delegations', label: 'มอบหมายสิทธิ์', count: delegations.length });
		}

		return tabs;
	});

	function positionLabel(positionCode: string, positionTitle?: string | null): string {
		if (positionTitle) return positionTitle;
		if (positionCode === 'director') return 'ผู้อำนวยการ';
		if (positionCode === 'deputy_director') return 'รองผู้อำนวยการ';
		if (positionCode === 'head') return 'หัวหน้า';
		if (positionCode === 'deputy_head') return 'รองหัวหน้า';
		if (positionCode === 'coordinator') return 'ผู้ประสานงาน';
		return 'สมาชิก';
	}

	function memberWorkText(member: OrganizationMemberItem): string {
		return member.responsibilities || positionLabel(member.position_code, member.position_title);
	}

	async function loadData(currentDeptId: string) {
		if (!canReadOrganization) {
			department = null;
			allDepartments = [];
			childDepts = [];
			deptMembers = [];
			error = 'ไม่มีสิทธิ์ดูข้อมูลหน่วยงาน';
			loading = false;
			return;
		}

		try {
			loading = true;
			error = '';
			delegations = [];
			delegatablePerms = [];
			const [deptRes, membersRes, allDeptsRes] = await Promise.all([
				getOrganizationUnit(currentDeptId),
				listOrganizationMembers(currentDeptId),
				listOrganizationUnits()
			]);
			if (currentDeptId !== deptId) return;
			if (deptRes.success && deptRes.data) {
				department = deptRes.data;
			} else {
				throw new Error(deptRes.error || 'OrganizationUnit not found');
			}
			if (membersRes.success && membersRes.data) {
				deptMembers = membersRes.data;
			}
			if (allDeptsRes.success && allDeptsRes.data) {
				allDepartments = allDeptsRes.data;
				childDepts = allDeptsRes.data
					.filter((unit) => unit.parent_unit_id === currentDeptId)
					.sort((a, b) => (a.display_order || 0) - (b.display_order || 0));
			}
		} catch (e: unknown) {
			if (currentDeptId !== deptId) return;
			error = (e instanceof Error ? e.message : String(e)) || 'Error loading data';
		} finally {
			if (currentDeptId === deptId) {
				loading = false;
			}
		}
	}

	async function loadDelegations(currentDeptId: string) {
		const [delRes, permRes] = await Promise.all([
			listDelegations(currentDeptId),
			listDelegatablePermissions(currentDeptId)
		]);
		if (currentDeptId !== deptId) return;
		if (delRes.success && delRes.data) delegations = delRes.data;
		if (permRes.success && permRes.data) delegatablePerms = permRes.data;
	}

	function refreshCurrentUnit() {
		if (!deptId) return;
		loadData(deptId);
	}

	function goToChildDept(id: string) {
		goto(resolve(`/staff/organization/${id}`));
	}

	async function handleRevoke(delegationId: string) {
		const res = await revokeDelegation(delegationId);
		if (res.success) {
			delegations = delegations.filter((delegation) => delegation.id !== delegationId);
		}
	}

	function closeDelegateDialog() {
		showDelegateDialog = false;
		delegateError = '';
	}

	function delegateMemberLabel(userId: string) {
		return deptMembers.find((member) => member.user_id === userId)?.name ?? 'เลือกสมาชิก';
	}

	function delegatePermissionLabel(permissionId: string) {
		const permission = delegatablePerms.find((item) => item.id === permissionId);
		if (!permission) return 'เลือกสิทธิ์';
		return `${permission.name} (${permission.code})`;
	}

	async function handleDelegate() {
		const currentDeptId = deptId;
		if (!currentDeptId || !delegateForm.to_user_id || !delegateForm.permission_id) return;
		delegateSubmitting = true;
		delegateError = '';
		try {
			const body: CreateDelegationBody = {
				to_user_id: delegateForm.to_user_id,
				permission_id: delegateForm.permission_id
			};
			if (delegateForm.reason) body.reason = delegateForm.reason;
			if (delegateForm.expires_at)
				body.expires_at = new Date(delegateForm.expires_at).toISOString();

			const res = await createDelegation(currentDeptId, body);
			if (res.success) {
				closeDelegateDialog();
				delegateForm = { to_user_id: '', permission_id: '', reason: '', expires_at: '' };
				await loadDelegations(currentDeptId);
			} else {
				delegateError = res.error || 'เกิดข้อผิดพลาด';
			}
		} finally {
			delegateSubmitting = false;
		}
	}

	$effect(() => {
		const currentDeptId = deptId;
		if (!currentDeptId) return;
		activeTab = 'members';
		loadData(currentDeptId);
	});

	$effect(() => {
		const currentDeptId = deptId;
		if (!loading && canManageDelegations && currentDeptId) {
			loadDelegations(currentDeptId);
		}
	});
</script>

<svelte:head>
	<title>{department ? department.name : 'รายละเอียดหน่วยงาน'} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-5">
	<div class="flex items-center gap-3">
		<Button href="/staff/organization" variant="ghost" size="sm" class="gap-2">
			<ArrowLeft class="h-4 w-4" />
			โครงสร้าง
		</Button>
	</div>

	{#if loading}
		<div class="rounded-lg border bg-card p-12 text-center text-sm text-muted-foreground">
			กำลังโหลดข้อมูล...
		</div>
	{:else if error}
		<div class="rounded-lg bg-destructive/10 p-6 text-sm text-destructive">{error}</div>
	{:else if department}
		<div class="space-y-5">
			<section class="rounded-lg border bg-card p-5">
				<div class="flex flex-col gap-5 xl:flex-row xl:items-start xl:justify-between">
					<div class="min-w-0 space-y-4">
						<div class="flex flex-wrap items-center gap-2">
							<Badge variant="outline">{department.code}</Badge>
							<Badge variant="secondary">{categoryText}</Badge>
							<Badge variant={department.unit_type === 'management_group' ? 'default' : 'outline'}>
								{unitTypeText}
							</Badge>
						</div>

						<div class="flex items-start gap-3">
							<div
								class="mt-1 flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
							>
								{#if department.category === 'academic'}
									<GraduationCap class="h-6 w-6" />
								{:else}
									<Briefcase class="h-6 w-6" />
								{/if}
							</div>
							<div class="min-w-0">
								<h1 class="break-words text-2xl font-bold text-foreground">{department.name}</h1>
								{#if department.name_en}
									<p class="mt-1 text-sm text-muted-foreground">{department.name_en}</p>
								{/if}
								<p class="mt-2 max-w-3xl text-sm leading-6 text-muted-foreground">
									{department.description || 'ยังไม่มีรายละเอียดหน่วยงาน'}
								</p>
							</div>
						</div>
					</div>

					<div class="flex flex-wrap gap-2">
						{#if canUpdateOrganizationUnit}
							<Button variant="outline" class="gap-2" onclick={() => (showEditDialog = true)}>
								<Pencil class="h-4 w-4" />
								แก้ไขหน่วยงาน
							</Button>
						{/if}
						{#if canReadOrganizationPermissions}
							<Button variant="outline" class="gap-2" onclick={() => (showPermissionDialog = true)}>
								<KeyRound class="h-4 w-4" />
								{canUpdateOrganizationPermissions ? 'สิทธิ์ตามตำแหน่ง' : 'ดูสิทธิ์ตามตำแหน่ง'}
							</Button>
						{/if}
						{#if canCreateOrganizationUnit}
							<Button class="gap-2" onclick={() => (showAddChildDialog = true)}>
								<Plus class="h-4 w-4" />
								เพิ่มหน่วยงานย่อย
							</Button>
						{/if}
					</div>
				</div>

				<div class="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
					{#each detailStats as stat (stat.label)}
						<div class="rounded-lg border bg-muted/20 p-4">
							<p class="text-xs font-medium text-muted-foreground">{stat.label}</p>
							<div class="mt-2 flex items-end justify-between gap-3">
								<p class="text-2xl font-semibold leading-none">{stat.value}</p>
								<p class="text-right text-xs text-muted-foreground">{stat.helper}</p>
							</div>
						</div>
					{/each}
				</div>
			</section>

			<div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
				<main class="min-w-0 space-y-4">
					<div class="flex gap-2 overflow-x-auto rounded-lg border bg-card p-1">
						{#each detailTabs as tab (tab.id)}
							<button
								type="button"
								class="flex shrink-0 items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors {activeTab ===
								tab.id
									? 'bg-primary text-primary-foreground'
									: 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
								onclick={() => (activeTab = tab.id)}
							>
								<span>{tab.label}</span>
								{#if tab.count !== undefined}
									<span
										class="rounded-full px-1.5 text-xs {activeTab === tab.id
											? 'bg-primary-foreground/20'
											: 'bg-muted'}">{tab.count}</span
									>
								{/if}
							</button>
						{/each}
					</div>

					{#if activeTab === 'members'}
						<OrganizationMembersSection
							organizationUnitId={deptId}
							childUnits={childDepts}
							canAssignMembers={canAssignOrganizationMembers}
							onChanged={refreshCurrentUnit}
						/>
					{:else if activeTab === 'permissions'}
						<section class="space-y-4 rounded-lg border bg-card p-5">
							<div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
								<div class="space-y-1">
									<h2 class="flex items-center gap-2 text-lg font-semibold">
										<KeyRound class="h-5 w-5 text-primary" />
										สิทธิ์ตามตำแหน่ง
									</h2>
									<p class="max-w-2xl text-sm text-muted-foreground">
										กำหนดสิทธิ์ตามตำแหน่งในหน่วยงานนี้ เช่น ผู้อำนวยการ หัวหน้า รองหัวหน้า และสมาชิก
									</p>
								</div>
								{#if canReadOrganizationPermissions}
									<Button class="gap-2" onclick={() => (showPermissionDialog = true)}>
										<KeyRound class="h-4 w-4" />
										{canUpdateOrganizationPermissions ? 'เปิดตารางสิทธิ์' : 'ดูตารางสิทธิ์'}
									</Button>
								{/if}
							</div>
							<div class="grid gap-3 sm:grid-cols-3">
								<div class="rounded-lg border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">หน่วยงาน</p>
									<p class="mt-1 truncate text-sm font-medium">{department.name}</p>
								</div>
								<div class="rounded-lg border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">รหัส</p>
									<p class="mt-1 font-mono text-sm font-medium">{department.code}</p>
								</div>
								<div class="rounded-lg border bg-muted/20 p-3">
									<p class="text-xs text-muted-foreground">ตำแหน่งหลัก</p>
									<p class="mt-1 text-sm font-medium">{leaderCount} รายการ</p>
								</div>
							</div>
						</section>
					{:else if activeTab === 'children'}
						<section class="space-y-4 rounded-lg border bg-card p-5">
							<div class="flex items-center justify-between gap-3">
								<div class="space-y-1">
									<h2 class="flex items-center gap-2 text-lg font-semibold">
										<Network class="h-5 w-5 text-primary" />
										หน่วยงานย่อย
									</h2>
									<p class="text-sm text-muted-foreground">
										หน่วยงานที่อยู่ภายใต้ {department.name}
									</p>
								</div>
								{#if canCreateOrganizationUnit}
									<Button size="sm" class="gap-2" onclick={() => (showAddChildDialog = true)}>
										<Plus class="h-4 w-4" />
										เพิ่ม
									</Button>
								{/if}
							</div>
							{#if childDepts.length === 0}
								<div
									class="rounded-lg border border-dashed py-10 text-center text-sm text-muted-foreground"
								>
									ยังไม่มีหน่วยงานย่อย
								</div>
							{:else}
								<div class="grid gap-3 md:grid-cols-2">
									{#each childDepts as child (child.id)}
										<button
											type="button"
											onclick={() => goToChildDept(child.id)}
											class="flex w-full items-center justify-between gap-3 rounded-lg border px-4 py-3 text-left transition-colors hover:border-primary/50 hover:bg-muted/30"
										>
											<div class="min-w-0">
												<p class="truncate text-sm font-medium">{child.name}</p>
												<p class="mt-1 font-mono text-xs text-muted-foreground">{child.code}</p>
											</div>
											<ArrowRight class="h-4 w-4 shrink-0 text-muted-foreground" />
										</button>
									{/each}
								</div>
							{/if}
						</section>
					{:else if activeTab === 'delegations' && canManageDelegations}
						<section class="space-y-4 rounded-lg border bg-card p-5">
							<div class="flex items-center justify-between gap-3">
								<div class="space-y-1">
									<h2 class="flex items-center gap-2 text-lg font-semibold">
										<Shield class="h-5 w-5 text-primary" />
										การมอบหมายสิทธิ์
									</h2>
									<p class="text-sm text-muted-foreground">
										มอบหมายสิทธิ์ชั่วคราวให้สมาชิกในหน่วยงานนี้
									</p>
								</div>
								<Button size="sm" class="gap-2" onclick={() => (showDelegateDialog = true)}>
									<Plus class="h-4 w-4" />
									มอบหมาย
								</Button>
							</div>

							{#if delegations.length === 0}
								<div
									class="rounded-lg border border-dashed py-10 text-center text-sm text-muted-foreground"
								>
									ยังไม่มีการมอบหมายสิทธิ์
								</div>
							{:else}
								<div class="divide-y rounded-lg border">
									{#each delegations as delegation (delegation.id)}
										<div class="flex items-start justify-between gap-4 px-4 py-3">
											<div class="min-w-0 space-y-0.5">
												<p class="truncate text-sm font-medium">{delegation.to_user_name}</p>
												<p class="text-xs text-muted-foreground">
													{delegation.permission_name}
													<span class="font-mono">({delegation.permission_code})</span>
												</p>
												{#if delegation.reason}
													<p class="text-xs text-muted-foreground">เหตุผล: {delegation.reason}</p>
												{/if}
												{#if delegation.expires_at}
													<p class="text-xs text-muted-foreground">
														หมดอายุ: {new Date(delegation.expires_at).toLocaleDateString('th-TH')}
													</p>
												{/if}
											</div>
											<Button
												variant="ghost"
												size="sm"
												onclick={() => handleRevoke(delegation.id)}
												class="shrink-0 text-destructive hover:text-destructive"
											>
												<Trash2 class="h-4 w-4" />
											</Button>
										</div>
									{/each}
								</div>
							{/if}
						</section>
					{/if}
				</main>

				<aside class="space-y-4">
					<section class="rounded-lg border bg-card p-5">
						<div class="flex items-center gap-2">
							<Info class="h-5 w-5 text-primary" />
							<h2 class="font-semibold">ข้อมูลหน่วยงาน</h2>
						</div>
						<div class="mt-4 space-y-3 text-sm">
							<div class="rounded-lg border bg-muted/20 p-3">
								<p class="text-xs text-muted-foreground">สังกัดภายใต้</p>
								<p class="mt-1 font-medium">{contextPanel.parentName}</p>
								{#if contextPanel.parentCode}
									<p class="mt-1 font-mono text-xs text-muted-foreground">
										{contextPanel.parentCode}
									</p>
								{/if}
							</div>
							{#each contactItems as item (item.key)}
								<div class="flex items-start gap-3 rounded-lg border px-3 py-2">
									{#if item.key === 'phone'}
										<Phone class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
									{:else if item.key === 'email'}
										<Mail class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
									{:else}
										<MapPin class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
									{/if}
									<div class="min-w-0">
										<p class="text-xs text-muted-foreground">{item.label}</p>
										<p class="break-words font-medium">{item.value}</p>
									</div>
								</div>
							{/each}
						</div>
					</section>

					<section class="rounded-lg border bg-card p-5">
						<div class="flex items-center justify-between gap-3">
							<div class="flex items-center gap-2">
								<Users class="h-5 w-5 text-primary" />
								<h2 class="font-semibold">งานหลักของหน่วยงาน</h2>
							</div>
							<Badge variant="outline">{contextPanel.memberCount} คน</Badge>
						</div>

						{#if primaryMembers.length === 0}
							<div
								class="mt-4 rounded-lg border border-dashed py-8 text-center text-sm text-muted-foreground"
							>
								ยังไม่มีผู้รับผิดชอบหลัก
							</div>
						{:else}
							<div class="mt-4 divide-y rounded-lg border">
								{#each primaryMembers.slice(0, 5) as member (member.user_id + '-' + member.organization_unit_id + '-' + member.position_code)}
									<div class="space-y-1 px-3 py-3">
										<div class="flex items-start justify-between gap-3">
											<div class="min-w-0">
												<p class="truncate text-sm font-medium">{member.name}</p>
												<p class="text-xs text-muted-foreground">
													{positionLabel(member.position_code, member.position_title)}
												</p>
											</div>
											{#if member.is_primary}
												<Badge variant="outline" class="shrink-0 text-[10px]">หลัก</Badge>
											{/if}
										</div>
										<p class="line-clamp-2 text-xs text-muted-foreground">
											{memberWorkText(member)}
										</p>
									</div>
								{/each}
							</div>
							{#if primaryMembers.length > 5}
								<p class="mt-2 text-xs text-muted-foreground">
									และอีก {primaryMembers.length - 5} คนในรายชื่อสมาชิก
								</p>
							{/if}
						{/if}
					</section>

					<section class="rounded-lg border bg-card p-5">
						<div class="flex items-center justify-between gap-3">
							<div class="flex items-center gap-2">
								<Network class="h-5 w-5 text-primary" />
								<h2 class="font-semibold">หน่วยงานย่อย</h2>
							</div>
							<Badge variant="outline">{contextPanel.childCount}</Badge>
						</div>
						{#if childDepts.length === 0}
							<p class="mt-4 text-sm text-muted-foreground">ยังไม่มีหน่วยงานย่อย</p>
						{:else}
							<div class="mt-4 space-y-2">
								{#each childDepts.slice(0, 4) as child (child.id)}
									<button
										type="button"
										onclick={() => goToChildDept(child.id)}
										class="flex w-full items-center justify-between gap-3 rounded-lg border px-3 py-2 text-left text-sm transition-colors hover:border-primary/50 hover:bg-muted/30"
									>
										<div class="min-w-0">
											<p class="truncate font-medium">{child.name}</p>
											<p class="font-mono text-xs text-muted-foreground">{child.code}</p>
										</div>
										<ArrowRight class="h-4 w-4 shrink-0 text-muted-foreground" />
									</button>
								{/each}
							</div>
							{#if childDepts.length > 4}
								<p class="mt-2 text-xs text-muted-foreground">
									และอีก {childDepts.length - 4} หน่วยงานในแท็บหน่วยงานย่อย
								</p>
							{/if}
						{/if}
					</section>
				</aside>
			</div>
		</div>
	{/if}
</div>

{#if canUpdateOrganizationUnit}
	<OrganizationUnitDialog
		bind:open={showEditDialog}
		organizationUnitToEdit={department}
		organizationUnits={allDepartments}
		onSuccess={refreshCurrentUnit}
	/>
{/if}

{#if canCreateOrganizationUnit}
	<OrganizationUnitDialog
		bind:open={showAddChildDialog}
		organizationUnits={allDepartments}
		forcedParentId={deptId}
		forcedCategory={department?.category}
		onSuccess={refreshCurrentUnit}
	/>
{/if}

{#if canReadOrganizationPermissions}
	<OrganizationPermissionDialog
		bind:open={showPermissionDialog}
		organizationUnit={department}
		onSuccess={refreshCurrentUnit}
		readOnly={!canUpdateOrganizationPermissions}
	/>
{/if}

<Dialog.Root bind:open={showDelegateDialog}>
	<Dialog.Content class="sm:max-w-[520px]">
		<Dialog.Header>
			<Dialog.Title>มอบหมายสิทธิ์</Dialog.Title>
			<Dialog.Description>มอบหมายสิทธิ์ชั่วคราวให้สมาชิกในหน่วยงานนี้</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-2">
			{#if delegateError}
				<div class="rounded bg-destructive/10 p-3 text-sm text-destructive">{delegateError}</div>
			{/if}

			<div class="space-y-2">
				<Label>สมาชิกที่จะมอบหมายให้ *</Label>
				<Select.Root type="single" bind:value={delegateForm.to_user_id}>
					<Select.Trigger class="w-full">
						{delegateMemberLabel(delegateForm.to_user_id)}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="">เลือกสมาชิก</Select.Item>
						{#each deptMembers as member (member.user_id + '-' + member.organization_unit_id)}
							<Select.Item value={member.user_id}>{member.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="space-y-2">
				<Label>สิทธิ์ที่มอบหมาย *</Label>
				<Select.Root type="single" bind:value={delegateForm.permission_id}>
					<Select.Trigger class="w-full">
						{delegatePermissionLabel(delegateForm.permission_id)}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="">เลือกสิทธิ์</Select.Item>
						{#each delegatablePerms as permission (permission.id)}
							<Select.Item value={permission.id}>{permission.name} ({permission.code})</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="space-y-2">
				<Label for="delegate-reason">
					เหตุผล <span class="font-normal text-muted-foreground">(ไม่บังคับ)</span>
				</Label>
				<Input
					id="delegate-reason"
					bind:value={delegateForm.reason}
					placeholder="เช่น ลาพักร้อน, รักษาการ"
				/>
			</div>

			<div class="space-y-2">
				<Label>
					วันหมดอายุ <span class="font-normal text-muted-foreground">(ไม่บังคับ)</span>
				</Label>
				<DatePicker bind:value={delegateForm.expires_at} placeholder="เลือกวันหมดอายุ" />
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={closeDelegateDialog}>ยกเลิก</Button>
			<Button
				onclick={handleDelegate}
				disabled={delegateSubmitting || !delegateForm.to_user_id || !delegateForm.permission_id}
			>
				{delegateSubmitting ? 'กำลังบันทึก...' : 'มอบหมาย'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
