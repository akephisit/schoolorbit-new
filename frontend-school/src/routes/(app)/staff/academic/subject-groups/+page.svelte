<script lang="ts">
	import { onMount } from 'svelte';
	import { listDepartments, type Department } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { GraduationCap, Users, ChevronRight, Search } from 'lucide-svelte';

	let departments: Department[] = $state([]);
	let loading = $state(true);
	let searchQuery = $state('');

	let subjectGroups = $derived(
		departments
			.filter((d) => {
				if (!d.code.startsWith('SUBJ-')) return false;
				if (!searchQuery) return true;
				const q = searchQuery.toLowerCase();
				return d.name.toLowerCase().includes(q) || d.code.toLowerCase().includes(q);
			})
			.sort((a, b) => (a.display_order || 0) - (b.display_order || 0))
	);

	async function loadData() {
		loading = true;
		const res = await listDepartments();
		if (res.success && res.data) departments = res.data;
		loading = false;
	}

	onMount(loadData);
</script>

<svelte:head>
	<title>กลุ่มสาระการเรียนรู้ - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-bold flex items-center gap-2">
				<GraduationCap class="w-7 h-7 text-orange-500" />
				กลุ่มสาระการเรียนรู้
			</h1>
			<p class="text-muted-foreground text-sm mt-1">จัดการกลุ่มสาระและสมาชิกในแต่ละกลุ่ม</p>
		</div>
	</div>

	<!-- Search -->
	<div class="relative max-w-sm">
		<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
		<input
			type="text"
			bind:value={searchQuery}
			placeholder="ค้นหากลุ่มสาระ..."
			class="w-full pl-9 pr-4 py-2 rounded-md border border-input bg-background text-sm"
		/>
	</div>

	{#if loading}
		<div class="p-12 text-center text-muted-foreground">กำลังโหลดข้อมูล...</div>
	{:else if subjectGroups.length === 0}
		<div class="p-12 text-center text-muted-foreground">ไม่พบกลุ่มสาระ</div>
	{:else}
		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
			{#each subjectGroups as group}
				<a
					href="/staff/academic/subject-groups/{group.id}"
					class="bg-card border border-border rounded-lg p-5 hover:border-primary/50 hover:shadow-sm transition-all group"
				>
					<div class="flex items-start justify-between">
						<div class="flex items-center gap-3">
							<div class="w-10 h-10 rounded-full bg-orange-100 dark:bg-orange-900/20 flex items-center justify-center">
								<GraduationCap class="w-5 h-5 text-orange-500" />
							</div>
							<div>
								<p class="font-semibold text-sm">{group.name}</p>
								<p class="text-xs text-muted-foreground">{group.code}</p>
							</div>
						</div>
						<ChevronRight class="w-4 h-4 text-muted-foreground group-hover:text-foreground transition-colors mt-1" />
					</div>
					{#if group.description}
						<p class="text-xs text-muted-foreground mt-3 line-clamp-2">{group.description}</p>
					{/if}
				</a>
			{/each}
		</div>
	{/if}
</div>
