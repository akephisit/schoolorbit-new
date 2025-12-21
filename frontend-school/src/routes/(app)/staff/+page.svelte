<script lang="ts">
	import { onMount } from 'svelte';
	import { listStaff, deleteStaff, type StaffListItem } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Users, Plus, Search, Pencil, Trash2, Eye } from 'lucide-svelte';

	let staffList: StaffListItem[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	let searchQuery = $state('');
	let currentPage = $state(1);
	let totalPages = $state(1);
	let total = $state(0);

	async function loadStaff() {
		try {
			loading = true;
			error = '';
			const response = await listStaff({
				search: searchQuery || undefined,
				page: currentPage,
				page_size: 20
			});

			staffList = response.data;
			total = response.total;
			totalPages = response.total_pages;
		} catch (e) {
			error = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
			console.error('Failed to load staff:', e);
		} finally {
			loading = false;
		}
	}

	async function handleDelete(id: string) {
		if (!confirm('คุณแน่ใจหรือไม่ที่จะลบบุคลากรนี้?')) return;

		try {
			await deleteStaff(id);
			await loadStaff();
		} catch (e) {
			alert('ไม่สามารถลบบุคลากรได้: ' + (e instanceof Error ? e.message : ''));
		}
	}

	function handleSearch() {
		currentPage = 1;
		loadStaff();
	}

	onMount(() => {
		loadStaff();
	});
</script>

<svelte:head>
	<title>จัดการบุคลากร - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
		<div>
			<h1 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<Users class="w-8 h-8" />
				จัดการบุคลากร
			</h1>
			<p class="text-muted-foreground mt-1">จัดการข้อมูลครูและบุคลากรทั้งหมด</p>
		</div>
		<Button href="/staff/new" class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			เพิ่มบุคลากร
		</Button>
	</div>

	<!-- Search Bar -->
	<div class="bg-card border border-border rounded-lg p-4">
		<div class="flex gap-2">
			<div class="flex-1 relative">
				<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
				<Input
					type="text"
					bind:value={searchQuery}
					onkeypress={(e) => e.key === 'Enter' && handleSearch()}
					placeholder="ค้นหาชื่อ, นามสกุล, รหัสพนักงาน..."
					class="pl-10"
				/>
			</div>
			<Button onclick={handleSearch}>ค้นหา</Button>
		</div>
	</div>

	<!-- Staff List -->
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
			<Button onclick={loadStaff} variant="outline" class="mt-4">ลองอีกครั้ง</Button>
		</div>
	{:else if staffList.length === 0}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<Users class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
			<p class="text-lg font-medium text-foreground">ไม่พบบุคลากร</p>
			<p class="text-muted-foreground mt-2">เริ่มต้นด้วยการเพิ่มบุคลากรคนแรก</p>
			<Button href="/staff/new" class="mt-4">
				<Plus class="w-4 h-4 mr-2" />
				เพิ่มบุคลากร
			</Button>
		</div>
	{:else}
		<div class="bg-card border border-border rounded-lg overflow-hidden">
			<!-- Table Header -->
			<div class="bg-muted/50 px-6 py-3 border-b border-border">
				<div class="grid grid-cols-12 gap-4 text-sm font-medium text-muted-foreground">
					<div class="col-span-3">ชื่อ-นามสกุล</div>
					<div class="col-span-2">รหัสพนักงาน</div>
					<div class="col-span-3">บทบาท</div>
					<div class="col-span-2">สถานะ</div>
					<div class="col-span-2 text-right">จัดการ</div>
				</div>
			</div>

			<!-- Table Body -->
			<div class="divide-y divide-border">
				{#each staffList as staff}
					<div class="px-6 py-4 hover:bg-accent/50 transition-colors">
						<div class="grid grid-cols-12 gap-4 items-center">
							<!-- Name -->
							<div class="col-span-3">
								<p class="font-medium text-foreground">
									{staff.first_name}
									{staff.last_name}
								</p>
							</div>

							<!-- Employee ID -->
							<div class="col-span-2">
								<p class="text-sm text-muted-foreground">
									{staff.employee_id || '-'}
								</p>
							</div>

							<!-- Roles -->
							<div class="col-span-3">
								<div class="flex flex-wrap gap-1">
									{#if staff.roles && staff.roles.length > 0}
										{#each staff.roles.slice(0, 2) as role}
											<span class="text-xs px-2 py-1 bg-primary/10 text-primary rounded-full">
												{role}
											</span>
										{/each}
										{#if staff.roles.length > 2}
											<span class="text-xs px-2 py-1 bg-muted text-muted-foreground rounded-full">
												+{staff.roles.length - 2}
											</span>
										{/if}
									{:else}
										<span class="text-sm text-muted-foreground">-</span>
									{/if}
								</div>
							</div>

							<!-- Status -->
							<div class="col-span-2">
								{#if staff.status === 'active'}
									<span
										class="inline-flex items-center text-xs px-2 py-1 bg-green-100 text-green-800 rounded-full"
									>
										<span class="w-1.5 h-1.5 rounded-full bg-green-500 mr-1.5"></span>
										ใช้งาน
									</span>
								{:else}
									<span
										class="inline-flex items-center text-xs px-2 py-1 bg-gray-100 text-gray-800 rounded-full"
									>
										<span class="w-1.5 h-1.5 rounded-full bg-gray-500 mr-1.5"></span>
										ไม่ใช้งาน
									</span>
								{/if}
							</div>

							<!-- Actions -->
							<div class="col-span-2 flex justify-end gap-2">
								<Button href="/staff/{staff.id}" variant="ghost" size="sm">
									<Eye class="w-4 h-4" />
								</Button>
								<Button href="/staff/{staff.id}/edit" variant="ghost" size="sm">
									<Pencil class="w-4 h-4" />
								</Button>
								<Button onclick={() => handleDelete(staff.id)} variant="ghost" size="sm">
									<Trash2 class="w-4 h-4 text-destructive" />
								</Button>
							</div>
						</div>
					</div>
				{/each}
			</div>

			<!-- Pagination -->
			{#if totalPages > 1}
				<div class="bg-muted/30 px-6 py-4 border-t border-border">
					<div class="flex items-center justify-between">
						<p class="text-sm text-muted-foreground">
							แสดง {staffList.length} จาก {total} รายการ
						</p>
						<div class="flex gap-2">
							<Button
								onclick={() => {
									currentPage--;
									loadStaff();
								}}
								disabled={currentPage === 1}
								variant="outline"
								size="sm"
							>
								← ก่อนหน้า
							</Button>
							<span class="px-4 py-2 text-sm">
								หน้า {currentPage} / {totalPages}
							</span>
							<Button
								onclick={() => {
									currentPage++;
									loadStaff();
								}}
								disabled={currentPage >= totalPages}
								variant="outline"
								size="sm"
							>
								ถัดไป →
							</Button>
						</div>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
