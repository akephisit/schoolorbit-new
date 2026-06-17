<script lang="ts">
	import { onMount } from 'svelte';
	import { listStaff, deleteStaff, type StaffListItem } from '$lib/api/staff';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '$lib/components/ui/dialog';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { PageShell } from '$lib/components/app-layout';
	import { LoadingButton, PageSkeleton, PageState } from '$lib/components/app-state';
	import { ChevronLeft, ChevronRight, Eye, Pencil, Plus, Search, Trash2 } from 'lucide-svelte';

	let { data } = $props();

	let staffList: StaffListItem[] = $state([]);
	let loading = $state(true);
	let deleting = $state(false);
	let showDeleteDialog = $state(false);
	let staffToDelete: StaffListItem | null = $state(null);
	let error = $state('');
	let searchQuery = $state('');
	let currentPage = $state(1);
	let totalPages = $state(1);
	let total = $state(0);

	const canReadStaff = $derived(
		$can.hasAny(
			PERMISSIONS.STAFF_PROFILE_READ_OWN,
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
			PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
			PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
		)
	);
	const canCreateStaff = $derived($can.has(PERMISSIONS.STAFF_CREATE_ALL));
	const canUpdateStaff = $derived($can.has(PERMISSIONS.STAFF_UPDATE_ALL));
	const canDeleteStaff = $derived($can.has(PERMISSIONS.STAFF_DELETE_ALL));

	async function loadStaff() {
		if (!canReadStaff) {
			staffList = [];
			total = 0;
			totalPages = 1;
			loading = false;
			error = '';
			return;
		}

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

	function openDeleteDialog(staff: StaffListItem) {
		if (!canDeleteStaff) return;
		staffToDelete = staff;
		showDeleteDialog = true;
	}

	async function confirmDelete() {
		if (!staffToDelete || !canDeleteStaff) return;

		deleting = true;
		try {
			await deleteStaff(staffToDelete.id);
			showDeleteDialog = false;
			staffToDelete = null;
			await loadStaff();
		} catch (e) {
			error = 'ไม่สามารถลบบุคลากรได้: ' + (e instanceof Error ? e.message : '');
			showDeleteDialog = false;
		} finally {
			deleting = false;
		}
	}

	function handleSearch() {
		currentPage = 1;
		void loadStaff();
	}

	function previousPage() {
		if (currentPage <= 1) return;
		currentPage -= 1;
		void loadStaff();
	}

	function nextPage() {
		if (currentPage >= totalPages) return;
		currentPage += 1;
		void loadStaff();
	}

	onMount(() => {
		void loadStaff();
	});
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<PageShell title="จัดการบุคลากร" description="จัดการข้อมูลครูและบุคลากรทั้งหมด">
	{#snippet actions()}
		{#if canCreateStaff}
			<Button href="/staff/manage/new" class="gap-2">
				<Plus class="h-4 w-4" />
				เพิ่มบุคลากร
			</Button>
		{/if}
	{/snippet}

	{#if !canReadStaff}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูรายชื่อบุคลากร"
			description="บัญชีนี้ยังไม่มีสิทธิ์อ่านข้อมูลบุคลากรในขอบเขตที่ระบบอนุญาต"
		/>
	{:else}
		<Card>
			<CardContent class="p-4">
				<div class="flex flex-col gap-2 sm:flex-row">
					<div class="relative flex-1">
						<Search
							class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
						/>
						<Input
							type="text"
							bind:value={searchQuery}
							onkeydown={(e) => e.key === 'Enter' && handleSearch()}
							placeholder="ค้นหาชื่อ, นามสกุล..."
							class="pl-10"
						/>
					</div>
					<Button onclick={handleSearch}>ค้นหา</Button>
				</div>
			</CardContent>
		</Card>

		{#if loading}
			<PageSkeleton variant="table" rows={6} columns={4} />
		{:else if error}
			<PageState
				variant="error"
				title="โหลดข้อมูลไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadStaff}
			/>
		{:else if staffList.length === 0}
			<PageState
				title="ไม่พบบุคลากร"
				description="ยังไม่มีรายการที่ตรงกับเงื่อนไขการค้นหา"
				actionLabel={canCreateStaff ? 'เพิ่มบุคลากร' : undefined}
				href={canCreateStaff ? '/staff/manage/new' : undefined}
			/>
		{:else}
			<Card>
				<CardHeader>
					<CardTitle>รายชื่อบุคลากร</CardTitle>
					<CardDescription>แสดง {staffList.length} จาก {total} รายการ</CardDescription>
				</CardHeader>
				<CardContent class="p-0">
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead>ชื่อ-นามสกุล</TableHead>
								<TableHead>บทบาท</TableHead>
								<TableHead>สถานะ</TableHead>
								<TableHead class="text-right">จัดการ</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{#each staffList as staff (staff.id)}
								<TableRow>
									<TableCell>
										<p class="font-medium text-foreground">
											{staff.title}{staff.first_name}
											{staff.last_name}
										</p>
										<p class="text-xs text-muted-foreground">{staff.username}</p>
									</TableCell>
									<TableCell>
										<div class="flex flex-wrap gap-1">
											{#if staff.roles && staff.roles.length > 0}
												{#each staff.roles.slice(0, 2) as role (role)}
													<Badge variant="secondary">{role}</Badge>
												{/each}
												{#if staff.roles.length > 2}
													<Badge variant="outline">+{staff.roles.length - 2}</Badge>
												{/if}
											{:else}
												<span class="text-sm text-muted-foreground">-</span>
											{/if}
										</div>
									</TableCell>
									<TableCell>
										<Badge variant={staff.status === 'active' ? 'default' : 'secondary'}>
											{staff.status === 'active' ? 'ใช้งาน' : 'ไม่ใช้งาน'}
										</Badge>
									</TableCell>
									<TableCell>
										<div class="flex justify-end gap-2">
											<Button
												href="/staff/manage/{staff.id}"
												variant="ghost"
												size="icon-sm"
												aria-label="ดูข้อมูล"
											>
												<Eye class="h-4 w-4" />
											</Button>
											{#if canUpdateStaff}
												<Button
													href="/staff/manage/{staff.id}/edit"
													variant="ghost"
													size="icon-sm"
													aria-label="แก้ไข"
												>
													<Pencil class="h-4 w-4" />
												</Button>
											{/if}
											{#if canDeleteStaff}
												<Button
													onclick={() => openDeleteDialog(staff)}
													variant="ghost"
													size="icon-sm"
													aria-label="ลบ"
												>
													<Trash2 class="h-4 w-4" />
												</Button>
											{/if}
										</div>
									</TableCell>
								</TableRow>
							{/each}
						</TableBody>
					</Table>
				</CardContent>
				{#if totalPages > 1}
					<div
						class="flex flex-col gap-3 border-t border-border px-6 py-4 sm:flex-row sm:items-center sm:justify-between"
					>
						<p class="text-sm text-muted-foreground">
							หน้า {currentPage} / {totalPages}
						</p>
						<div class="flex gap-2">
							<Button
								onclick={previousPage}
								disabled={currentPage === 1}
								variant="outline"
								size="sm"
								class="gap-2"
							>
								<ChevronLeft class="h-4 w-4" />
								ก่อนหน้า
							</Button>
							<Button
								onclick={nextPage}
								disabled={currentPage >= totalPages}
								variant="outline"
								size="sm"
								class="gap-2"
							>
								ถัดไป
								<ChevronRight class="h-4 w-4" />
							</Button>
						</div>
					</div>
				{/if}
			</Card>
		{/if}
	{/if}
</PageShell>

<Dialog bind:open={showDeleteDialog}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>ยืนยันการลบบุคลากร</DialogTitle>
			<DialogDescription>
				คุณแน่ใจหรือไม่ว่าต้องการลบบุคลากร
				{#if staffToDelete}
					<strong>
						{staffToDelete.first_name}
						{staffToDelete.last_name}
					</strong>
				{/if}? การกระทำนี้จะทำให้บุคลากรถูกปิดการใช้งาน
			</DialogDescription>
		</DialogHeader>
		<DialogFooter>
			<Button variant="outline" onclick={() => (showDeleteDialog = false)} disabled={deleting}>
				ยกเลิก
			</Button>
			<LoadingButton
				variant="destructive"
				onclick={confirmDelete}
				loading={deleting}
				loadingLabel="กำลังลบ..."
				class="gap-2"
			>
				<Trash2 class="h-4 w-4" />
				ลบบุคลากร
			</LoadingButton>
		</DialogFooter>
	</DialogContent>
</Dialog>
