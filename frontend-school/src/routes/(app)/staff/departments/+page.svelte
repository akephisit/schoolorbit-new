<script lang="ts">
	import { onMount } from 'svelte';
	import { listDepartments, updateDepartment } from '$lib/api/staff';
	import type { Department } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Badge } from '$lib/components/ui/badge';
	import {
		Building2,
		Plus,
		Pencil,
		Search,
		Phone,
		Mail,
		MapPin,
		Briefcase,
		GraduationCap,
		Settings
	} from 'lucide-svelte';
	import * as Select from '$lib/components/ui/select';
	import DepartmentDialog from '$lib/components/staff/DepartmentDialog.svelte';
	import DepartmentPermissionDialog from '$lib/components/staff/DepartmentPermissionDialog.svelte'; // New import
	import { toast } from 'svelte-sonner';

	let departments: Department[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	let searchQuery = $state('');

	// Drag and Drop State
	let draggedDeptId: string | null = $state(null);
	let dragOverDeptId: string | null = $state(null);

	let showDialog = $state(false);
	let editingDepartment: Department | null = $state(null);

	// Permission Dialog State
	let showPermissionDialog = $state(false);
	let permissionDepartment = $state<Department | null>(null);

	// Hierarchical Data Processing
	// Filter logic: Only Administrative or Miscellaneous
	let filteredDepartments = $derived(
		departments.filter((dept) => {
			const query = searchQuery.toLowerCase();
			const matchesSearch =
				dept.name.toLowerCase().includes(query) ||
				dept.code.toLowerCase().includes(query) ||
				(dept.name_en && dept.name_en.toLowerCase().includes(query));

			const isNotSubjectGroup = !dept.code.startsWith('SUBJ-');

			return matchesSearch && isNotSubjectGroup;
		})
	);

	let isSearching = $derived(searchQuery.length > 0);

	let rootDepartments = $derived(
		isSearching
			? []
			: departments
					.filter((d) => !d.parent_department_id && !d.code.startsWith('SUBJ-'))
					.sort((a, b) => (a.display_order || 0) - (b.display_order || 0))
	);

	function getChildren(parentId: string): Department[] {
		return departments
			.filter((d) => d.parent_department_id === parentId)
			.sort((a, b) => (a.display_order || 0) - (b.display_order || 0));
	}

	async function loadDepartments() {
		try {
			loading = true;
			error = '';
			const response = await listDepartments();

			if (response.success && response.data) {
				departments = response.data;
			} else {
				error = response.error || 'ไม่สามารถโหลดข้อมูลฝ่ายได้';
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

	function handlePermission(dept: Department) {
		permissionDepartment = dept;
		showPermissionDialog = true;
	}

	// Drag and Drop Handlers
	function handleDragStart(e: DragEvent, deptId: string) {
		e.stopPropagation(); // Prevent bubbling to parent
		if (!e.dataTransfer) return;
		draggedDeptId = deptId;
		e.dataTransfer.effectAllowed = 'move';
		e.dataTransfer.setData('text/plain', deptId);
	}

	function handleDragEnd(e: DragEvent) {
		draggedDeptId = null;
		dragOverDeptId = null;
	}

	function handleDragOver(e: DragEvent, deptId: string) {
		e.preventDefault(); // allow drop
		e.stopPropagation();

		// If dragging over itself or one of its children (circular), we should ideally prevent it.
		// For now simple check:
		if (draggedDeptId === deptId) return;

		dragOverDeptId = deptId;
	}

	function handleDragLeave(e: DragEvent) {
		// e.stopPropagation();
	}

	async function handleDrop(e: DragEvent, targetParentId: string | null) {
		e.preventDefault();
		e.stopPropagation();
		dragOverDeptId = null;
		const sourceDeptId = e.dataTransfer?.getData('text/plain');

		if (!sourceDeptId) return;

		// Prevent dropping on itself
		if (sourceDeptId === targetParentId) return;

		const sourceDept = departments.find((d) => d.id === sourceDeptId);
		const targetDept = targetParentId ? departments.find((d) => d.id === targetParentId) : null;

		if (!sourceDept) return;

		// Validation: Prevent nesting deeper than 2 levels
		// Case: Moving a parent (Dept with children) into another parent (creating level 3)
		if (targetParentId && getChildren(sourceDept.id).length > 0) {
			toast.error('ไม่สามารถย้ายฝ่ายที่มีฝ่ายย่อยไปอยู่ใต้ฝ่ายอื่นได้ (จำกัดโครงสร้าง 2 ระดับ)');
			return;
		}

		// Direct move without confirmation
		const loadingToast = toast.loading('กำลังย้ายฝ่าย...');
		try {
			const result = await updateDepartment(sourceDeptId, {
				parent_department_id: targetParentId ?? undefined
			});

			if (result.success) {
				toast.success('ย้ายฝ่ายสำเร็จ', { id: loadingToast });
				loadDepartments();
			} else {
				toast.error('ย้ายฝ่ายไม่สำเร็จ: ' + result.error, { id: loadingToast });
			}
		} catch (err: any) {
			toast.error('เกิดข้อผิดพลาด: ' + err.message, { id: loadingToast });
		}
	}

	onMount(() => {
		loadDepartments();
	});
</script>

<svelte:head>
	<title>จัดการฝ่าย - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
		<div>
			<h1 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<Building2 class="w-8 h-8" />
				จัดการฝ่าย
			</h1>
			<p class="text-muted-foreground mt-1">จัดการโครงสร้างองค์กรและฝ่ายบริหารทั่วไป</p>
		</div>
		<Button onclick={handleCreate} class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			เพิ่มฝ่าย
		</Button>
	</div>

	<!-- Search & Filter Bar (No Category Select) -->
	<div class="flex flex-col sm:flex-row gap-4">
		<div class="bg-card border border-border rounded-lg p-1 flex-1">
			<div class="relative">
				<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
				<Input
					type="text"
					bind:value={searchQuery}
					placeholder="ค้นหาฝ่าย..."
					class="pl-10 border-0 focus-visible:ring-0"
				/>
			</div>
		</div>
	</div>

	<!-- Departments List -->
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
	{:else if isSearching}
		<!-- Search Results Mode (Flat Grid) -->
		{#if filteredDepartments.length === 0}
			<div class="bg-card border border-border rounded-lg p-12 text-center">
				<p class="text-lg font-medium text-foreground">ไม่พบฝ่ายที่ค้นหา</p>
			</div>
		{:else}
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
				{#each filteredDepartments as dept (dept.id)}
					<!-- Reusing the Card UI in a flat structure -->
					<div class="bg-card border border-border rounded-lg p-4 relative group">
						<div class="flex items-center gap-2 mb-2">
							<Badge variant="outline">{dept.code}</Badge>
							<span class="font-semibold">{dept.name}</span>
						</div>
						<div class="text-sm text-muted-foreground mb-4 line-clamp-2">
							{dept.description || '-'}
						</div>
						<Button variant="outline" size="sm" class="w-full" onclick={() => handleEdit(dept)}>
							<Pencil class="w-3 h-3 mr-2" /> แก้ไข
						</Button>
					</div>
				{/each}
			</div>
		{/if}
	{:else}
		<!-- Hierarchical Mode (Nested Cards) -->
		<div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6 items-start">
			{#each rootDepartments as root (root.id)}
				{@const children = getChildren(root.id)}

				<!-- Root/Parent Card -->
				<div
					class="bg-muted/40 border border-border rounded-xl p-4 flex flex-col gap-4
                               transition-all duration-200
                               {dragOverDeptId === root.id
						? 'ring-2 ring-primary bg-primary/10'
						: ''}"
					ondragover={(e) => handleDragOver(e, root.id)}
					ondrop={(e) => handleDrop(e, root.id)}
					role="group"
				>
					<!-- Parent Header -->
					<div class="flex items-start justify-between">
						<div class="flex-1">
							<div class="flex items-center gap-2 mb-1">
								{#if root.category === 'academic'}
									<GraduationCap class="w-5 h-5 text-orange-500" />
								{:else}
									<Briefcase class="w-5 h-5 text-blue-500" />
								{/if}
								<h3 class="font-bold text-lg text-foreground">
									<a href="/staff/departments/{root.id}" class="hover:underline">{root.name}</a>
								</h3>
							</div>
							{#if root.name_en}<p class="text-xs text-muted-foreground ml-7">
									{root.name_en}
								</p>{/if}
						</div>
						<div class="flex items-center gap-1">
							<Button
								variant="ghost"
								size="icon"
								class="h-8 w-8 text-muted-foreground hover:text-foreground"
								onclick={() => handlePermission(root)}
								title="จัดการสิทธิ์เมนู"
							>
								<Settings class="w-4 h-4" />
							</Button>
							<Button variant="ghost" size="icon" class="h-8 w-8" onclick={() => handleEdit(root)}>
								<Pencil class="w-3 h-3" />
							</Button>
						</div>
					</div>

					<!-- Children Container -->
					<div class="flex flex-col gap-2 min-h-[50px]">
						<!-- Snippet for Recursive Children -->
						{#snippet departmentNode(dept: Department)}
							<div
								class="bg-card border border-border/60 hover:border-primary/50 shadow-sm rounded-lg p-3
									   cursor-move transition-all group relative list-item-card
									   {draggedDeptId === dept.id ? 'opacity-40' : ''}"
								draggable="true"
								role="listitem"
								ondragstart={(e) => handleDragStart(e, dept.id)}
								ondragend={handleDragEnd}
							>
								<div class="flex items-center justify-between gap-2">
									<div class="flex items-center gap-2 overflow-hidden">
										<div
											class="w-1.5 h-8 rounded-full {dept.is_active
												? 'bg-green-500'
												: 'bg-gray-300'} shrink-0"
										></div>
										<div class="flex flex-col truncate">
											<span class="font-medium truncate text-sm">
												<a href="/staff/departments/{dept.id}" class="hover:underline"
													>{dept.name}</a
												>
											</span>
											<span class="text-[10px] text-muted-foreground flex gap-2">
												<span>{dept.code}</span>
												{#if dept.phone}<span>• 📞 {dept.phone}</span>{/if}
											</span>
										</div>
									</div>
									<div
										class="flex items-center gap-0 opacity-0 group-hover:opacity-100 transition-opacity"
									>
										<Button
											variant="ghost"
											size="icon"
											class="h-7 w-7 text-muted-foreground hover:text-foreground"
											onclick={() => handlePermission(dept)}
											title="จัดการสิทธิ์เมนู"
										>
											<Settings class="w-3.5 h-3.5" />
										</Button>
										<Button
											variant="ghost"
											size="icon"
											class="h-7 w-7"
											onclick={() => handleEdit(dept)}
										>
											<Pencil class="w-3.5 h-3.5 text-muted-foreground" />
										</Button>
									</div>
								</div>
							</div>

							<!-- Recursive GrandChildren (Display only, no drop handlers) -->
							{@const grandChildren = getChildren(dept.id)}
							{#if grandChildren.length > 0}
								<div
									class="ml-6 pl-2 border-l-2 border-dashed border-border/30 flex flex-col gap-2 pt-2"
								>
									{#each grandChildren as grandChild (grandChild.id)}
										{@render departmentNode(grandChild)}
									{/each}
								</div>
							{/if}
						{/snippet}

						<!-- Render First Level Children -->
						{#each children as child (child.id)}
							{@render departmentNode(child)}
						{/each}

						{#if children.length === 0}
							<div
								class="text-center py-4 border-2 border-dashed border-border/50 rounded-lg text-muted-foreground/50 text-xs"
							>
								ลากฝ่ายย่อยมาวางที่นี่
							</div>
						{/if}
					</div>
				</div>
			{/each}

			<!-- Add New Placeholders or Empty State if no roots -->
			{#if rootDepartments.length === 0}
				<div class="col-span-full py-12 text-center text-muted-foreground">
					ไม่พบข้อมูลฝ่าย
					<Button variant="link" onclick={handleCreate}>เพิ่มฝ่ายใหม่</Button>
				</div>
			{/if}
		</div>
	{/if}

	<DepartmentDialog
		bind:open={showDialog}
		departmentToEdit={editingDepartment}
		{departments}
		onSuccess={loadDepartments}
		forcedCategory="administrative"
	/>

	<DepartmentPermissionDialog bind:open={showPermissionDialog} department={permissionDepartment} />
</div>
