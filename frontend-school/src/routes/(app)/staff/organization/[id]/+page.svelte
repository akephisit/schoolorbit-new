<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
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
	import { PERMISSION_MODULES, PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import OrganizationMembersSection from '$lib/components/staff/OrganizationMembersSection.svelte';
	import {
		ArrowLeft,
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
		Info
	} from 'lucide-svelte';
	import OrganizationPermissionDialog from '$lib/components/staff/OrganizationPermissionDialog.svelte';

	const { params }: PageProps = $props();
	type DetailTab = 'overview' | 'members' | 'permissions' | 'children' | 'delegations';

	let deptId = $derived(params.id);
	let department: OrganizationUnit | null = $state(null);
	let allDepartments: OrganizationUnit[] = $state([]);
	let childDepts: OrganizationUnit[] = $state([]);
	let deptMembers: OrganizationMemberItem[] = $state([]);
	let delegations: DelegationItem[] = $state([]);
	let activeTab = $state<DetailTab>('overview');
	let showPermissionDialog = $state(false);

	// Child dept dialog
	let showAddChildDialog = $state(false);
	let delegatablePerms: DelegatablePermission[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	// Delegation form state
	let showDelegateDialog = $state(false);
	let delegateForm = $state({ to_user_id: '', permission_id: '', reason: '', expires_at: '' });
	let delegateSubmitting = $state(false);
	let delegateError = $state('');

	let leaderCount = $derived(
		deptMembers.filter((member) =>
			['director', 'deputy_director', 'head'].includes(member.position_code)
		).length
	);

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
			{ id: 'overview', label: 'ภาพรวม' },
			{ id: 'members', label: 'สมาชิก', count: deptMembers.length },
			{ id: 'permissions', label: 'สิทธิ์' },
			{ id: 'children', label: 'หน่วยงานย่อย', count: childDepts.length }
		];

		if ($can.has(PERMISSIONS.ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT)) {
			tabs.push({ id: 'delegations', label: 'มอบหมาย', count: delegations.length });
		}

		return tabs;
	});

	async function loadData() {
		if (!deptId) return;
		try {
			loading = true;
			const [deptRes, membersRes, allDeptsRes] = await Promise.all([
				getOrganizationUnit(deptId),
				listOrganizationMembers(deptId),
				listOrganizationUnits()
			]);
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
					.filter((d) => d.parent_unit_id === deptId)
					.sort((a, b) => (a.display_order || 0) - (b.display_order || 0));
			}
		} catch (e: unknown) {
			error = (e instanceof Error ? e.message : String(e)) || 'Error loading data';
		} finally {
			loading = false;
		}
	}

	async function loadDelegations() {
		if (!deptId) return;
		const [delRes, permRes] = await Promise.all([
			listDelegations(deptId),
			listDelegatablePermissions(deptId)
		]);
		if (delRes.success && delRes.data) delegations = delRes.data;
		if (permRes.success && permRes.data) delegatablePerms = permRes.data;
	}

	function goToChildDept(id: string) {
		goto(resolve(`/staff/organization/${id}`));
	}

	async function handleRevoke(delegationId: string) {
		const res = await revokeDelegation(delegationId);
		if (res.success) {
			delegations = delegations.filter((d) => d.id !== delegationId);
		}
	}

	async function handleDelegate() {
		if (!deptId || !delegateForm.to_user_id || !delegateForm.permission_id) return;
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

			const res = await createDelegation(deptId, body);
			if (res.success) {
				showDelegateDialog = false;
				delegateForm = { to_user_id: '', permission_id: '', reason: '', expires_at: '' };
				await loadDelegations();
			} else {
				delegateError = res.error || 'เกิดข้อผิดพลาด';
			}
		} finally {
			delegateSubmitting = false;
		}
	}

	onMount(() => {
		loadData();
	});

	$effect(() => {
		if (!loading && $can.has(PERMISSIONS.ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT) && deptId) {
			loadDelegations();
		}
	});
</script>

<svelte:head>
	<title>{department ? department.name : 'รายละเอียดหน่วยงาน'} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header / Back -->
	<div class="flex items-center gap-4">
		<Button href="/staff/organization" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4" />
		</Button>
		<div class="flex-1">
			<h1 class="text-2xl font-bold text-foreground flex items-center gap-2">
				{#if loading}
					กำลังโหลด...
				{:else if department}
					{#if department.category === 'academic'}
						<GraduationCap class="w-8 h-8 text-orange-500" />
					{:else}
						<Briefcase class="w-8 h-8 text-blue-500" />
					{/if}
					{department.name}
				{:else}
					ไม่พบข้อมูล
				{/if}
			</h1>
			{#if department?.name_en}
				<p class="text-muted-foreground ml-10">{department.name_en}</p>
			{/if}
		</div>
	</div>

	{#if loading}
		<div class="p-12 text-center text-muted-foreground">กำลังโหลดข้อมูล...</div>
	{:else if error}
		<div class="p-6 bg-destructive/10 text-destructive rounded-lg">{error}</div>
	{:else if department}
		<div class="space-y-5">
			<div class="rounded-lg border bg-card p-5">
				<div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
					<div class="min-w-0 space-y-3">
						<div class="flex flex-wrap items-center gap-2">
							<Badge variant="outline">{department.code}</Badge>
							<Badge variant="secondary">{categoryText}</Badge>
							<Badge variant={department.unit_type === 'management_group' ? 'default' : 'outline'}>
								{unitTypeText}
							</Badge>
						</div>
						<p class="max-w-3xl text-sm text-muted-foreground">
							{department.description || 'ยังไม่มีรายละเอียด'}
						</p>
					</div>

					<div class="flex flex-wrap gap-2">
						{#if $can.hasModule(PERMISSION_MODULES.ROLES)}
							<Button variant="outline" onclick={() => (showPermissionDialog = true)}>
								<KeyRound class="mr-2 h-4 w-4" />
								สิทธิ์ตามตำแหน่ง
							</Button>
						{/if}
						{#if $can.has(PERMISSIONS.ROLES_ASSIGN_ALL)}
							<Button variant="outline" onclick={() => (showAddChildDialog = true)}>
								<Plus class="mr-2 h-4 w-4" />
								เพิ่มหน่วยงาน
							</Button>
						{/if}
					</div>
				</div>

				<div class="mt-5 grid grid-cols-2 gap-3 lg:grid-cols-4">
					<div class="rounded-md border bg-muted/20 p-3">
						<p class="text-xs text-muted-foreground">สมาชิก</p>
						<p class="text-xl font-semibold">{deptMembers.length}</p>
					</div>
					<div class="rounded-md border bg-muted/20 p-3">
						<p class="text-xs text-muted-foreground">ผู้บริหาร/หัวหน้า</p>
						<p class="text-xl font-semibold">{leaderCount}</p>
					</div>
					<div class="rounded-md border bg-muted/20 p-3">
						<p class="text-xs text-muted-foreground">หน่วยงานย่อย</p>
						<p class="text-xl font-semibold">{childDepts.length}</p>
					</div>
					<div class="rounded-md border bg-muted/20 p-3">
						<p class="text-xs text-muted-foreground">มอบหมายสิทธิ์</p>
						<p class="text-xl font-semibold">{delegations.length}</p>
					</div>
				</div>
			</div>

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

			{#if activeTab === 'overview'}
				<div class="grid grid-cols-1 gap-5 lg:grid-cols-2">
					<div class="rounded-lg border bg-card p-5 space-y-4">
						<h2 class="flex items-center gap-2 text-lg font-semibold">
							<Info class="h-5 w-5" />
							ข้อมูลทั่วไป
						</h2>
						<div class="grid gap-4 sm:grid-cols-2">
							<div>
								<span class="text-sm text-muted-foreground">รหัสหน่วยงาน</span>
								<p class="font-medium">{department.code}</p>
							</div>
							<div>
								<span class="text-sm text-muted-foreground">ประเภท</span>
								<p class="font-medium">{categoryText} · {unitTypeText}</p>
							</div>
							<div class="sm:col-span-2">
								<span class="text-sm text-muted-foreground">รายละเอียด</span>
								<p class="mt-1">{department.description || '-'}</p>
							</div>
						</div>
					</div>

					<div class="rounded-lg border bg-card p-5 space-y-4">
						<h2 class="flex items-center gap-2 text-lg font-semibold">
							<Phone class="h-5 w-5" />
							การติดต่อ
						</h2>
						<div class="grid gap-3">
							<div class="flex items-center gap-3">
								<Phone class="h-4 w-4 text-muted-foreground" />
								<span class="text-sm">{department.phone || '-'}</span>
							</div>
							<div class="flex items-center gap-3">
								<Mail class="h-4 w-4 text-muted-foreground" />
								<span class="text-sm">{department.email || '-'}</span>
							</div>
							<div class="flex items-center gap-3">
								<MapPin class="h-4 w-4 text-muted-foreground" />
								<span class="text-sm">{department.location || '-'}</span>
							</div>
						</div>
					</div>
				</div>
			{:else if activeTab === 'members'}
				<OrganizationMembersSection
					organizationUnitId={deptId}
					childUnits={childDepts}
					onChanged={loadData}
				/>
			{:else if activeTab === 'permissions'}
				<div class="rounded-lg border bg-card p-6">
					<div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
						<div class="space-y-1">
							<h2 class="flex items-center gap-2 text-lg font-semibold">
								<KeyRound class="h-5 w-5" />
								สิทธิ์ตามตำแหน่ง
							</h2>
							<p class="text-sm text-muted-foreground">
								{department.name}
							</p>
						</div>
						<Button onclick={() => (showPermissionDialog = true)}>
							<KeyRound class="mr-2 h-4 w-4" />
							เปิดตารางสิทธิ์
						</Button>
					</div>
				</div>
			{:else if activeTab === 'children'}
				<div class="rounded-lg border bg-card p-6 space-y-4">
					<div class="flex items-center justify-between">
						<h2 class="flex items-center gap-2 text-lg font-semibold">
							<Network class="h-5 w-5" />
							หน่วยงานย่อย
						</h2>
						{#if $can.has(PERMISSIONS.ROLES_ASSIGN_ALL)}
							<Button size="sm" onclick={() => (showAddChildDialog = true)}>
								<Plus class="mr-1 h-4 w-4" />
								เพิ่มหน่วยงาน
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
									onclick={() => goToChildDept(child.id)}
									class="flex w-full items-center justify-between rounded-md border px-4 py-3 text-left transition-colors hover:border-primary/50 hover:bg-muted/30"
								>
									<div class="min-w-0">
										<p class="truncate text-sm font-medium">{child.name}</p>
										<p class="font-mono text-xs text-muted-foreground">{child.code}</p>
									</div>
									<Briefcase class="h-4 w-4 text-muted-foreground" />
								</button>
							{/each}
						</div>
					{/if}
				</div>
			{:else if activeTab === 'delegations'}
				<div class="rounded-lg border bg-card p-6 space-y-4">
					<div class="flex items-center justify-between">
						<h2 class="flex items-center gap-2 text-lg font-semibold">
							<Shield class="h-5 w-5" />
							การมอบหมายสิทธิ์
						</h2>
						<Button size="sm" onclick={() => (showDelegateDialog = true)}>
							<Plus class="mr-1 h-4 w-4" />
							มอบหมายสิทธิ์
						</Button>
					</div>

					{#if delegations.length === 0}
						<div
							class="rounded-lg border border-dashed py-10 text-center text-sm text-muted-foreground"
						>
							ยังไม่มีการมอบหมายสิทธิ์
						</div>
					{:else}
						<div class="divide-y divide-border">
							{#each delegations as d (d.id)}
								<div class="flex items-start justify-between gap-4 py-3">
									<div class="space-y-0.5">
										<p class="text-sm font-medium">{d.to_user_name}</p>
										<p class="text-xs text-muted-foreground">
											{d.permission_name} <span class="font-mono">({d.permission_code})</span>
										</p>
										{#if d.reason}
											<p class="text-xs text-muted-foreground">เหตุผล: {d.reason}</p>
										{/if}
										{#if d.expires_at}
											<p class="text-xs text-muted-foreground">
												หมดอายุ: {new Date(d.expires_at).toLocaleDateString('th-TH')}
											</p>
										{/if}
									</div>
									<Button
										variant="ghost"
										size="sm"
										onclick={() => handleRevoke(d.id)}
										class="shrink-0 text-destructive hover:text-destructive"
									>
										<Trash2 class="h-4 w-4" />
									</Button>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	{/if}
</div>

<OrganizationUnitDialog
	bind:open={showAddChildDialog}
	organizationUnits={allDepartments}
	forcedParentId={deptId}
	forcedCategory={department?.category}
	onSuccess={loadData}
/>

<OrganizationPermissionDialog
	bind:open={showPermissionDialog}
	organizationUnit={department}
	onSuccess={loadData}
/>

<!-- Delegate Permission Dialog -->
{#if showDelegateDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div
			class="bg-background border border-border rounded-xl shadow-lg w-full max-w-md p-6 space-y-4"
		>
			<h3 class="text-lg font-semibold">มอบหมายสิทธิ์</h3>

			{#if delegateError}
				<div class="text-sm text-destructive bg-destructive/10 rounded p-3">{delegateError}</div>
			{/if}

			<div class="space-y-3">
				<div class="space-y-1">
					<label for="delegate-to" class="text-sm font-medium">สมาชิกที่จะมอบหมายให้ *</label>
					<select
						id="delegate-to"
						bind:value={delegateForm.to_user_id}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						<option value="">-- เลือกสมาชิก --</option>
						{#each deptMembers as m (m.user_id + '-' + m.organization_unit_id)}
							<option value={m.user_id}>{m.name}</option>
						{/each}
					</select>
				</div>

				<div class="space-y-1">
					<label for="delegate-permission" class="text-sm font-medium">สิทธิ์ที่มอบหมาย *</label>
					<select
						id="delegate-permission"
						bind:value={delegateForm.permission_id}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						<option value="">-- เลือกสิทธิ์ --</option>
						{#each delegatablePerms as p (p.id)}
							<option value={p.id}>{p.name} ({p.code})</option>
						{/each}
					</select>
				</div>

				<div class="space-y-1">
					<label for="delegate-reason" class="text-sm font-medium"
						>เหตุผล <span class="text-muted-foreground font-normal">(ไม่บังคับ)</span></label
					>
					<input
						id="delegate-reason"
						type="text"
						bind:value={delegateForm.reason}
						placeholder="เช่น ลาพักร้อน, รักษาการ"
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					/>
				</div>

				<div class="space-y-1">
					<label for="delegate-expires" class="text-sm font-medium"
						>วันหมดอายุ <span class="text-muted-foreground font-normal">(ไม่บังคับ)</span></label
					>
					<input
						id="delegate-expires"
						type="date"
						bind:value={delegateForm.expires_at}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					/>
				</div>
			</div>

			<div class="flex justify-end gap-2 pt-2">
				<Button
					variant="outline"
					onclick={() => {
						showDelegateDialog = false;
						delegateError = '';
					}}
				>
					ยกเลิก
				</Button>
				<Button
					onclick={handleDelegate}
					disabled={delegateSubmitting || !delegateForm.to_user_id || !delegateForm.permission_id}
				>
					{delegateSubmitting ? 'กำลังบันทึก...' : 'มอบหมาย'}
				</Button>
			</div>
		</div>
	</div>
{/if}
