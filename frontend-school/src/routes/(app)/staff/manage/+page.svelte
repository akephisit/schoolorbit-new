<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import type { PageProps } from './$types';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
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
	import { ChevronLeft, ChevronRight, Eye, Pencil, Plus, Search, Trash2 } from '@lucide/svelte';

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

	let { data }: PageProps = $props();
	const listKey = $derived(data.listKey),
		source = $derived(data.staff);
	const staffRequest = new LatestRequest();
	let activeKey = '',
		disposed = false,
		mutationEpoch = 0;
	let loaded = $state(false);
	$effect.pre(() => {
		const key = listKey,
			read = source;
		untrack(() => {
			if (key !== activeKey) {
				activeKey = key;
				mutationEpoch++;
				staffList = [];
				loaded = false;
				showDeleteDialog = false;
				staffToDelete = null;
				deleting = false;
			}
			searchQuery = data.search;
			currentPage = data.page;
			const ticket = staffRequest.begin();
			loading = true;
			error = '';
			void read.then((result) => applyStaff(result, ticket.revision));
		});
		return () => staffRequest.abort();
	});
	onDestroy(() => {
		disposed = true;
		mutationEpoch++;
		staffRequest.abort();
	});
	function applyStaff(result: Awaited<typeof data.staff>, revision: number) {
		if (!staffRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			staffList = result.data?.data ?? [];
			total = result.data?.total ?? 0;
			totalPages = result.data?.total_pages ?? 1;
			currentPage = result.data?.page ?? data.page;
			loaded = result.data !== null;
		} else error = result.error;
	}
	async function loadStaff() {
		if (!canReadStaff) return;
		const ticket = staffRequest.begin();
		loading = true;
		error = '';
		applyStaff(
			await captureRouteLoad(
				listStaff(data.query, { signal: ticket.signal }),
				'โหลดรายชื่อบุคลากรไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	function openDeleteDialog(staff: StaffListItem) {
		if (!canDeleteStaff || deleting) return;
		staffToDelete = staff;
		showDeleteDialog = true;
	}
	async function confirmDelete() {
		if (!staffToDelete || !canDeleteStaff || deleting) return;
		const target = staffToDelete.id,
			key = listKey,
			epoch = ++mutationEpoch;
		const current = () => !disposed && epoch === mutationEpoch && key === listKey;
		staffRequest.abort();
		loading = false;
		deleting = true;
		try {
			const response = await deleteStaff(target);
			if (!current()) return;
			if (!response.success) throw new Error(response.error || 'ไม่สามารถลบบุคลากรได้');
			showDeleteDialog = false;
			staffToDelete = null;
			await loadStaff();
		} catch (e) {
			if (current()) {
				error = 'ไม่สามารถลบบุคลากรได้: ' + (e instanceof Error ? e.message : 'เกิดข้อผิดพลาด');
				showDeleteDialog = false;
			}
		} finally {
			if (current()) deleting = false;
		}
	}
	function navigatePage(nextPage: number, search = data.search) {
		if (deleting || nextPage < 1) return;
		const query = new SvelteURLSearchParams(page.url.search);
		if (search) query.set('search', search);
		else query.delete('search');
		if (nextPage > 1) query.set('page', String(nextPage));
		else query.delete('page');
		if (query.toString() === page.url.searchParams.toString()) void loadStaff();
		else void goto(resolve(`/staff/manage?${query}`));
	}
	function handleSearch() {
		navigatePage(1, searchQuery);
	}
	function previousPage() {
		if (currentPage > 1) navigatePage(currentPage - 1);
	}
	function nextPage() {
		if (currentPage < totalPages) navigatePage(currentPage + 1);
	}
</script>

<PageShell title="จัดการบุคลากร" description="จัดการข้อมูลครูและบุคลากรทั้งหมด">
	{#snippet actions()}
		<Button variant="outline" onclick={loadStaff} disabled={loading || deleting || !canReadStaff}
			>รีเฟรช</Button
		>
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
		<Card class="gap-0 py-0">
			<CardContent class="p-3 sm:p-4">
				<div class="flex flex-col gap-3 sm:flex-row">
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

		<section data-testid="staff-directory" aria-busy={loading}>
			{#if error && loaded}<PageState
					title="อัปเดตรายชื่อไม่สำเร็จ"
					description={error}
					actionLabel="ลองอีกครั้ง"
					onaction={loadStaff}
				/>{/if}
			{#if loading && loaded}<p role="status">กำลังอัปเดตรายชื่อ...</p>{/if}
			{#if loading && !loaded}<div role="status" aria-label="กำลังโหลดรายชื่อบุคลากร">
					<PageSkeleton variant="table" rows={6} columns={4} />
				</div>
			{:else if error && !loaded}
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
													data-sveltekit-preload-data="tap"
													variant="ghost"
													size="icon-sm"
													aria-label="ดูข้อมูล"
												>
													<Eye class="h-4 w-4" />
												</Button>
												{#if canUpdateStaff}
													<Button
														href="/staff/manage/{staff.id}/edit"
														data-sveltekit-preload-data="tap"
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
		</section>
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
