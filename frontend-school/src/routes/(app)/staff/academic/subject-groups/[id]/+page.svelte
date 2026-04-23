<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { getDepartmentLookup, type Department } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import DeptMembersSection from '$lib/components/staff/DeptMembersSection.svelte';
	import DepartmentPermissionDialog from '$lib/components/staff/DepartmentPermissionDialog.svelte';
	import { can } from '$lib/stores/permissions';
	import { GraduationCap, ArrowLeft, Building2, Settings } from 'lucide-svelte';

	const { params }: PageProps = $props();
	let deptId = $derived(params.id);
	let department: Department | null = $state(null);
	let loading = $state(true);
	let error = $state('');

	let showPermissionDialog = $state(false);

	async function loadData() {
		if (!deptId) return;
		try {
			loading = true;
			const res = await getDepartmentLookup(deptId);
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
		{#if department && $can.hasAny('roles.assign.all', '*')}
			<Button variant="outline" size="sm" onclick={() => (showPermissionDialog = true)}>
				<Settings class="w-4 h-4 mr-1" />
				ตั้งค่าสิทธิ์
			</Button>
		{/if}
	</div>

	{#if loading}
		<div class="p-12 text-center text-muted-foreground">กำลังโหลดข้อมูล...</div>
	{:else if error}
		<div class="p-6 bg-destructive/10 text-destructive rounded-lg">{error}</div>
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
				<DeptMembersSection departmentId={deptId} />
			</div>
		</div>
	{/if}
</div>

<DepartmentPermissionDialog bind:open={showPermissionDialog} {department} />
