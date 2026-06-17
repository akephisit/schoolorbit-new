<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { getOrganizationUnitLookup, type OrganizationUnit } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import OrganizationMembersSection from '$lib/components/staff/OrganizationMembersSection.svelte';
	import OrganizationPermissionDialog from '$lib/components/staff/OrganizationPermissionDialog.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { GraduationCap, ArrowLeft, Building2, Settings } from 'lucide-svelte';

	const { params }: PageProps = $props();
	let deptId = $derived(params.id);
	let department: OrganizationUnit | null = $state(null);
	let loading = $state(true);
	let error = $state('');

	let showPermissionDialog = $state(false);

	const canReadSubjectGroupDetail = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CURRICULUM_READ_ALL,
			PERMISSIONS.ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE
		)
	);
	const canManageSubjectGroupPermissions = $derived($can.has(PERMISSIONS.ROLES_ASSIGN_ALL));

	async function loadData() {
		if (!deptId) return;
		if (!canReadSubjectGroupDetail) {
			department = null;
			loading = false;
			return;
		}

		try {
			loading = true;
			error = '';
			const res = await getOrganizationUnitLookup(deptId);
			if (res.success && res.data) {
				department = res.data;
			} else {
				throw new Error(res.error || 'ไม่พบกลุ่มสาระ');
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
		} finally {
			loading = false;
		}
	}

	onMount(loadData);
</script>

<svelte:head>
	<title>{department ? department.name : 'รายละเอียดกลุ่มสาระ'} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-4">
		<Button href="/staff/academic/subject-groups" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4" />
		</Button>
		<div class="flex-1">
			<h1 class="text-2xl font-bold flex items-center gap-2">
				{#if loading}
					กำลังโหลด...
				{:else if department}
					<GraduationCap class="w-7 h-7 text-orange-500" />
					{department.name}
				{:else}
					ไม่พบข้อมูล
				{/if}
			</h1>
			{#if department}
				<p class="text-muted-foreground text-sm">{department.code}</p>
			{/if}
		</div>
		{#if department && canManageSubjectGroupPermissions}
			<Button variant="outline" size="sm" onclick={() => (showPermissionDialog = true)}>
				<Settings class="w-4 h-4 mr-1" />
				ตั้งค่าสิทธิ์
			</Button>
		{/if}
	</div>

	{#if !canReadSubjectGroupDetail}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูรายละเอียดกลุ่มสาระ"
			description="บัญชีนี้เข้า module หลักสูตรได้ แต่ยังไม่มีสิทธิ์อ่านรายละเอียดกลุ่มสาระ"
		/>
	{:else if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดรายละเอียดกลุ่มสาระไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadData}
		/>
	{:else if department}
		<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
			<!-- Left: Info -->
			<div class="md:col-span-2 space-y-6">
				<div class="bg-card border border-border rounded-lg p-6 space-y-4">
					<h2 class="text-lg font-semibold flex items-center gap-2">
						<Building2 class="w-5 h-5" />
						ข้อมูลทั่วไป
					</h2>
					<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
						<div>
							<span class="text-sm text-muted-foreground">รหัสกลุ่ม</span>
							<p class="font-medium">{department.code}</p>
						</div>
						<div>
							<span class="text-sm text-muted-foreground">ประเภท</span>
							<div class="mt-1">
								<Badge variant="outline">กลุ่มสาระการเรียนรู้</Badge>
							</div>
						</div>
						<div class="col-span-2">
							<span class="text-sm text-muted-foreground">รายละเอียด</span>
							<p class="mt-1">{department.description || '-'}</p>
						</div>
					</div>
				</div>
			</div>

			<!-- Right: Members -->
			<div>
				<OrganizationMembersSection
					organizationUnitId={deptId}
					canAssignMembers={canManageSubjectGroupPermissions}
				/>
			</div>
		</div>
	{:else}
		<PageState
			title="ไม่พบกลุ่มสาระ"
			description="กลุ่มสาระนี้อาจถูกลบหรือคุณอาจไม่มีสิทธิ์เข้าถึง"
			actionLabel="กลับหน้ากลุ่มสาระ"
			href="/staff/academic/subject-groups"
		/>
	{/if}
</div>

<OrganizationPermissionDialog
	bind:open={showPermissionDialog}
	organizationUnit={department}
	readOnly={!canManageSubjectGroupPermissions}
/>
