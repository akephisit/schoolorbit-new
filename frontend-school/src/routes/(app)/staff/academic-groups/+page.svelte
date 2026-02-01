<script lang="ts">
	import { onMount } from 'svelte';
	import { listDepartments } from '$lib/api/staff';
	import type { Department } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Badge } from '$lib/components/ui/badge';
	import { Plus, Pencil, Search, Library, GraduationCap, MoreHorizontal } from 'lucide-svelte';
	import DepartmentDialog from '$lib/components/staff/DepartmentDialog.svelte';
	import { toast } from 'svelte-sonner';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';

	let departments: Department[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	let searchQuery = $state('');

	let showDialog = $state(false);
	let editingDepartment: Department | null = $state(null);

	// Filter Logic: Only Academic, no hierarchy needed for display (flat list is fine for simple management)
	// But usually users want to see simple list.
	let filteredDepartments = $derived(
		departments
			.filter((dept) => {
				const query = searchQuery.toLowerCase();
				const matchesSearch =
					dept.name.toLowerCase().includes(query) ||
					dept.code.toLowerCase().includes(query) ||
					(dept.name_en && dept.name_en.toLowerCase().includes(query));

				const isAllowedCategory = dept.category === 'academic';

				return matchesSearch && isAllowedCategory;
			})
			.sort((a, b) => (a.display_order || 0) - (b.display_order || 0))
	);

	async function loadDepartments() {
		try {
			loading = true;
			error = '';
			const response = await listDepartments();

			if (response.success && response.data) {
				departments = response.data;
			} else {
				error = response.error || 'ไม่สามารถโหลดข้อมูลกลุ่มสาระได้';
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
			console.error('Failed to load departments:', e);
		} finally {
			loading = false;
		}
	}

	function handleCreate() {
		editingDepartment = null;
		showDialog = true;
	}

	function handleEdit(dept: Department) {
		editingDepartment = dept;
		showDialog = true;
	}

	onMount(() => {
		loadDepartments();
	});
</script>

<svelte:head>
	<title>จัดการกลุ่มสาระฯ - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
		<div>
			<h1 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<Library class="w-8 h-8" />
				จัดการกลุ่มสาระฯ
			</h1>
			<p class="text-muted-foreground mt-1">จัดการกลุ่มสาระการเรียนรู้และหน่วยงานวิชาการ</p>
		</div>
		<Button onclick={handleCreate} class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			เพิ่มกลุ่มสาระ
		</Button>
	</div>

	<!-- Search Bar -->
	<div class="flex flex-col sm:flex-row gap-4">
		<div class="bg-card border border-border rounded-lg p-1 flex-1">
			<div class="relative">
				<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
				<Input
					type="text"
					bind:value={searchQuery}
					placeholder="ค้นหากลุ่มสาระ..."
					class="pl-10 border-0 focus-visible:ring-0"
				/>
			</div>
		</div>
	</div>

	<!-- Grid List -->
	{#if loading}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<div
				class="inline-block w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"
			></div>
			<p class="mt-4 text-muted-foreground">กำลังโหลด...</p>
		</div>
	{:else if error}
		<div class="bg-destructive/10 border border-destructive/20 rounded-lg p-6 text-center">
			<p class="text-destructive">{error}</p>
			<Button onclick={loadDepartments} variant="outline" class="mt-4">ลองอีกครั้ง</Button>
		</div>
	{:else if filteredDepartments.length === 0}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<p class="text-lg font-medium text-foreground">ไม่พบกลุ่มสาระที่ค้นหา</p>
			<Button variant="link" onclick={handleCreate}>เพิ่มกลุ่มสาระใหม่</Button>
		</div>
	{:else}
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
			{#each filteredDepartments as dept (dept.id)}
				<div
					class="bg-card border border-border rounded-lg p-5 relative group hover:shadow-md transition-shadow"
				>
					<div class="flex items-start justify-between mb-2">
						<div class="flex items-center gap-2">
							<GraduationCap class="w-5 h-5 text-primary" />
							<Badge variant="outline" class="bg-background">{dept.code}</Badge>
						</div>
						<DropdownMenu.Root>
							<DropdownMenu.Trigger>
								<div
									class="inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium ring-offset-background transition-colors disabled:pointer-events-none disabled:opacity-50 hover:bg-accent hover:text-accent-foreground h-8 w-8 -mr-2 cursor-pointer"
								>
									<MoreHorizontal class="w-4 h-4" />
								</div>
							</DropdownMenu.Trigger>
							<DropdownMenu.Content align="end">
								<DropdownMenu.Item onclick={() => handleEdit(dept)}>
									<Pencil class="w-4 h-4 mr-2" /> แก้ไข
								</DropdownMenu.Item>
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					</div>

					<a href="/staff/academic-groups/{dept.id}" class="block group-hover:underline">
						<h3 class="font-bold text-lg text-foreground mb-1">{dept.name}</h3>
					</a>

					{#if dept.name_en}
						<p class="text-sm text-muted-foreground mb-2">{dept.name_en}</p>
					{/if}

					{#if dept.description}
						<p
							class="text-xs text-muted-foreground line-clamp-2 mt-2 pt-2 border-t border-border/50"
						>
							{dept.description}
						</p>
					{/if}

					<div class="mt-4 flex items-center justify-between text-xs text-muted-foreground">
						<span>ลำดับ: {dept.display_order}</span>
					</div>
				</div>
			{/each}
		</div>
	{/if}

	<DepartmentDialog
		bind:open={showDialog}
		departmentToEdit={editingDepartment}
		{departments}
		onSuccess={loadDepartments}
		forcedCategory="academic"
	/>
</div>
