<script lang="ts">
	import { onMount } from 'svelte';
	import { listDepartments, type Department } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Badge } from '$lib/components/ui/badge';
	import { 
		Building2, Plus, Pencil, Search, Phone, Mail, MapPin, 
		Briefcase, GraduationCap, LayoutGrid, Layers, Users 
	} from 'lucide-svelte';
	import * as Select from '$lib/components/ui/select';

	let departments: Department[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	let searchQuery = $state('');
	let selectedCategory = $state('all'); // all, administrative, academic

	let filteredDepartments = $derived(
		departments.filter(
			(dept) => {
				// Filter by search
				const matchesSearch = dept.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
					dept.code.toLowerCase().includes(searchQuery.toLowerCase()) ||
					(dept.name_en && dept.name_en.toLowerCase().includes(searchQuery.toLowerCase()));
				
				// Filter by category
				const matchesCategory = selectedCategory === 'all' || dept.category === selectedCategory;

				return matchesSearch && matchesCategory;
			}
		)
	);

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
			<p class="text-muted-foreground mt-1">จัดการโครงสร้างองค์กรและหน่วยงาน</p>
		</div>
		<Button disabled class="flex items-center gap-2 opacity-50 cursor-not-allowed">
			<Plus class="w-4 h-4" />
			เพิ่มฝ่าย (ติดต่อผู้ดูแลระบบ)
		</Button>
	</div>

	<!-- Search Bar -->
	<!-- Search & Filter Bar -->
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

		<div class="w-full sm:w-[200px]">
			<Select.Root type="single" bind:value={selectedCategory}>
				<Select.Trigger>
					{selectedCategory === 'all'
						? 'ทั้งหมด'
						: selectedCategory === 'administrative'
							? 'บริหารจัดการ'
							: 'วิชาการ'}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="all">ทั้งหมด</Select.Item>
					<Select.Item value="administrative">บริหารจัดการ</Select.Item>
					<Select.Item value="academic">วิชาการ</Select.Item>
				</Select.Content>
			</Select.Root>
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
	{:else if filteredDepartments.length === 0}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<Building2 class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
			<p class="text-lg font-medium text-foreground">ไม่พบฝ่าย</p>
			<p class="text-muted-foreground mt-2">ลองค้นหาด้วยคำอื่น</p>
		</div>
	{:else}
		<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
			{#each filteredDepartments as dept (dept.id)}
				<div class="bg-card border border-border rounded-lg p-6 hover:shadow-md transition-shadow">
					<div class="flex items-start justify-between mb-4">
						<div class="flex-1">
							<div class="flex items-start justify-between gap-2 mb-1">
								<div class="flex items-center gap-2">
									{#if dept.category === 'academic'}
										<GraduationCap class="w-5 h-5 text-orange-500" />
									{:else}
										<Briefcase class="w-5 h-5 text-blue-500" />
									{/if}
									<h3 class="text-lg font-semibold text-foreground">{dept.name}</h3>
								</div>

								<div class="flex gap-1 flex-wrap justify-end">
									{#if dept.org_type === 'group'}
										<Badge variant="default" class="bg-slate-800 hover:bg-slate-900">Group</Badge>
									{:else}
										<Badge variant="secondary">Unit</Badge>
									{/if}
								</div>
							</div>
							{#if dept.name_en}
								<p class="text-sm text-muted-foreground ml-7 mb-2">{dept.name_en}</p>
							{/if}
						</div>
					</div>

					<div class="space-y-3">
						<div class="flex items-center gap-2">
							<span class="text-xs px-2.5 py-1 bg-primary/10 text-primary rounded-full font-medium">
								{dept.code}
							</span>
							<span
								class="text-xs px-2.5 py-1 bg-muted text-muted-foreground rounded-full font-medium"
							>
								ลำดับ: {dept.display_order}
							</span>
						</div>

						{#if dept.description}
							<p class="text-sm text-muted-foreground">{dept.description}</p>
						{/if}

						<!-- Contact Info -->
						<div class="pt-3 border-t border-border space-y-2">
							{#if dept.phone}
								<div class="flex items-center gap-2 text-sm">
									<Phone class="w-4 h-4 text-muted-foreground" />
									<span class="text-foreground">{dept.phone}</span>
								</div>
							{/if}

							{#if dept.email}
								<div class="flex items-center gap-2 text-sm">
									<Mail class="w-4 h-4 text-muted-foreground" />
									<span class="text-foreground">{dept.email}</span>
								</div>
							{/if}

							{#if dept.location}
								<div class="flex items-center gap-2 text-sm">
									<MapPin class="w-4 h-4 text-muted-foreground" />
									<span class="text-foreground">{dept.location}</span>
								</div>
							{/if}

							{#if !dept.phone && !dept.email && !dept.location}
								<p class="text-xs text-muted-foreground">ไม่มีข้อมูลการติดต่อ</p>
							{/if}
						</div>

						<!-- Status & Date -->
						<div class="flex items-center justify-between pt-2 border-t border-border">
							<span class="text-xs {dept.is_active ? 'text-green-600' : 'text-gray-500'}">
								{dept.is_active ? '● ใช้งาน' : '○ ไม่ใช้งาน'}
							</span>
							<span class="text-xs text-muted-foreground">
								สร้าง: {new Date(dept.created_at).toLocaleDateString('th-TH')}
							</span>
						</div>

						{#if dept.parent_department_id}
							<div class="text-xs text-muted-foreground">สังกัดภายใต้ฝ่ายแม่</div>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
