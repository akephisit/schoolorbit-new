<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { listOrganizationUnitsLookup, type OrganizationUnitLookupItem } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { ChevronRight, GraduationCap, Search, Settings } from 'lucide-svelte';
	import OrganizationPermissionDialog from '$lib/components/staff/OrganizationPermissionDialog.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	let departments: OrganizationUnitLookupItem[] = $state([]);
	let loading = $state(true);
	let searchQuery = $state('');

	let subjectGroups = $derived(
		departments
			.filter((d) => {
				if (d.unit_type !== 'subject_group' && !d.subject_group_id) return false;
				if (!searchQuery) return true;
				const q = searchQuery.toLowerCase();
				return d.name.toLowerCase().includes(q) || d.code.toLowerCase().includes(q);
			})
			.sort((a, b) => (a.display_order || 0) - (b.display_order || 0))
	);

	let showPermissionDialog = $state(false);
	let permissionDepartment = $state<OrganizationUnitLookupItem | null>(null);

	const canReadSubjectGroups = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CURRICULUM_READ_ALL,
			PERMISSIONS.ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE
		)
	);
	const canManageSubjectGroupPermissions = $derived($can.has(PERMISSIONS.ROLES_ASSIGN_ALL));

	function goToSubjectGroup(id: string) {
		if (!canReadSubjectGroups) return;
		goto(resolve(`/staff/academic/subject-groups/${id}`));
	}

	function handlePermission(dept: OrganizationUnitLookupItem, e: MouseEvent) {
		e.preventDefault();
		if (!canManageSubjectGroupPermissions) return;
		permissionDepartment = dept;
		showPermissionDialog = true;
	}

	async function loadData() {
		if (!canReadSubjectGroups) {
			departments = [];
			loading = false;
			return;
		}

		loading = true;
		// admin เห็นทุกกลุ่ม, user ทั่วไปเห็นเฉพาะกลุ่มที่ตัวเองสังกัด
		const isAdmin = canManageSubjectGroupPermissions;
		const res = await listOrganizationUnitsLookup(isAdmin ? undefined : { member_only: true });
		if (res.success && res.data) departments = res.data;
		loading = false;
	}

	onMount(loadData);
</script>

<svelte:head>
	<title>กลุ่มสาระการเรียนรู้ - SchoolOrbit</title>
</svelte:head>

<PageShell title="กลุ่มสาระการเรียนรู้" description="จัดการกลุ่มสาระและสมาชิกในแต่ละกลุ่ม">
	<!-- Search -->
	<div class="relative max-w-sm">
		<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
		<Input type="text" bind:value={searchQuery} placeholder="ค้นหากลุ่มสาระ..." class="pl-9" />
	</div>

	{#if !canReadSubjectGroups}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูกลุ่มสาระ"
			description="บัญชีนี้เข้า module หลักสูตรได้ แต่ยังไม่มีสิทธิ์อ่านกลุ่มสาระการเรียนรู้"
		/>
	{:else if loading}
		<PageSkeleton variant="cards" rows={6} />
	{:else if subjectGroups.length === 0}
		<PageState title="ไม่พบกลุ่มสาระ" description="ยังไม่มีกลุ่มสาระที่ตรงกับเงื่อนไขการค้นหา" />
	{:else}
		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
			{#each subjectGroups as group (group.id)}
				<div
					onclick={() => goToSubjectGroup(group.id)}
					role="button"
					tabindex="0"
					onkeydown={(e) => e.key === 'Enter' && goToSubjectGroup(group.id)}
					class="bg-card border border-border rounded-lg p-5 hover:border-primary/50 hover:shadow-sm transition-all group cursor-pointer"
				>
					<div class="flex items-start justify-between">
						<div class="flex items-center gap-3">
							<div
								class="w-10 h-10 rounded-full bg-orange-100 dark:bg-orange-900/20 flex items-center justify-center"
							>
								<GraduationCap class="w-5 h-5 text-orange-500" />
							</div>
							<div>
								<p class="font-semibold text-sm">{group.name}</p>
								<p class="text-xs text-muted-foreground">{group.code}</p>
							</div>
						</div>
						<div class="flex items-center gap-1">
							{#if canManageSubjectGroupPermissions}
								<Button
									variant="ghost"
									size="icon"
									class="h-7 w-7 text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 transition-opacity"
									onclick={(e) => handlePermission(group, e)}
									title="จัดการสิทธิ์"
								>
									<Settings class="w-3.5 h-3.5" />
								</Button>
							{/if}
							<ChevronRight
								class="w-4 h-4 text-muted-foreground group-hover:text-foreground transition-colors mt-1"
							/>
						</div>
					</div>
					{#if group.description}
						<p class="text-xs text-muted-foreground mt-3 line-clamp-2">{group.description}</p>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</PageShell>

<OrganizationPermissionDialog
	bind:open={showPermissionDialog}
	organizationUnit={permissionDepartment}
	readOnly={!canManageSubjectGroupPermissions}
/>
