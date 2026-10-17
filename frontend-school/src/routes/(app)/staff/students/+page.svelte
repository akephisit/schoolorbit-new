<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { listStudents, deleteStudent, type StudentListItem } from '#lib/api/students.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import {
		AlertDialog as Dialog,
		AlertDialogCancel as DialogCancel,
		AlertDialogContent as DialogContent,
		AlertDialogDescription as DialogDescription,
		AlertDialogFooter as DialogFooter,
		AlertDialogHeader as DialogHeader,
		AlertDialogTitle as DialogTitle
	} from '#lib/components/ui/alert-dialog/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import {
		REGISTRATION_STATUS_LABELS,
		studentRegisterReadScope,
		STUDENT_ACCOUNT_STATUS_LABELS,
		accountStatusLabel
	} from '#lib/forms/student-register.js';
	import {
		studentRegistryHref,
		withStudentRegisterContext
	} from '#lib/navigation/student-register.js';
	import { authStore } from '#lib/stores/auth.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Plus, Search, Pencil, Trash2, Eye, GraduationCap } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	const readScope = $derived(
		studentRegisterReadScope({
			school: $can.has(PERMISSIONS.STUDENT_READ_SCHOOL),
			assigned: $can.has(PERMISSIONS.STUDENT_READ_ASSIGNED),
			own: $can.has(PERMISSIONS.STUDENT_READ_OWN),
			piiSchool: $can.has(PERMISSIONS.STUDENT_PII_READ_SCHOOL),
			piiAssigned: $can.has(PERMISSIONS.STUDENT_PII_READ_ASSIGNED),
			piiOwn: $can.has(PERMISSIONS.STUDENT_PII_READ_OWN)
		})
	);

	const canReadStudents = $derived(
		$can.hasAny(
			PERMISSIONS.STUDENT_READ_SCHOOL,
			PERMISSIONS.STUDENT_READ_ASSIGNED,
			PERMISSIONS.STUDENT_READ_OWN
		)
	);
	const canCreateStudent = $derived($can.has(PERMISSIONS.STUDENT_CREATE_ALL));
	const canUpdateStudent = $derived($can.has(PERMISSIONS.STUDENT_UPDATE_ALL));
	const canDeleteStudent = $derived($can.has(PERMISSIONS.STUDENT_DELETE_ALL));
	let { data }: PageProps = $props();
	const listKey = $derived(data.listKey);
	const source = $derived(data.students);
	const studentsRequest = new LatestRequest();
	let activeKey = '';
	let disposed = false;
	let mutationEpoch = 0;
	let students = $state.raw<StudentListItem[]>([]);
	let loaded = $state(false);
	let loading = $state(true);
	let loadError = $state('');
	let deleting = $state(false);
	let showDeleteDialog = $state(false);
	let studentToDelete = $state<StudentListItem | null>(null);
	let mutationError = $state('');
	let searchQuery = $state('');
	let statusFilter = $state('all');
	let accountFilter = $state('all');
	let currentPage = $state(1);
	let hasNextPage = $state(false);
	const hasAppliedFilters = $derived(
		Boolean(data.search) || data.registrationStatus !== 'all' || data.accountStatus !== 'all'
	);
	const returnTo = $derived(studentRegistryHref(page.url));
	const createHref = $derived(
		withStudentRegisterContext('/staff/students/new', {
			returnTo,
			academicYearId: null,
			source: 'registry'
		})
	);
	$effect.pre(() => {
		if (!canDeleteStudent)
			untrack(() => {
				mutationEpoch++;
				showDeleteDialog = false;
				studentToDelete = null;
				deleting = false;
				mutationError = '';
			});
	});
	let previousIdentity = '';
	$effect.pre(() => {
		const identity = `${$authStore.user?.id ?? ''}:${readScope}`;
		untrack(() => {
			if (previousIdentity && previousIdentity !== identity) {
				mutationEpoch++;
				studentsRequest.abort();
				students = [];
				loaded = false;
				loading = true;
				loadError = '';
				showDeleteDialog = false;
				studentToDelete = null;
			}
			previousIdentity = identity;
		});
	});
	$effect.pre(() => {
		const key = listKey,
			read = source;
		untrack(() => {
			if (key !== activeKey) {
				activeKey = key;
				mutationEpoch++;
				students = [];
				loaded = false;
				deleting = false;
				showDeleteDialog = false;
				studentToDelete = null;
			}
			searchQuery = data.search;
			statusFilter = data.registrationStatus;
			accountFilter = data.accountStatus;
			currentPage = data.page;
			const ticket = studentsRequest.begin();
			loading = true;
			loadError = '';
			const session = authStore.sessionEpoch,
				scope = readScope;
			void read.then((result) => {
				if (session === authStore.sessionEpoch && scope === readScope && canReadStudents)
					applyStudents(result, ticket.revision);
			});
		});
		return () => studentsRequest.abort();
	});
	onDestroy(() => {
		disposed = true;
		mutationEpoch++;
		studentsRequest.abort();
	});
	function applyStudents(result: Awaited<typeof data.students>, revision: number) {
		if (!studentsRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			students = result.data?.items ?? [];
			loaded = result.data !== null;
			currentPage = result.data?.page ?? data.page;
			hasNextPage = result.data ? result.data.items.length === result.data.page_size : false;
		} else loadError = result.error;
	}

	async function reloadRegistry() {
		if (!canReadStudents) return;
		const ticket = studentsRequest.begin();
		loading = true;
		loadError = '';
		const result = await captureRouteLoad(
			listStudents(data.query, { signal: ticket.signal }),
			'โหลดรายชื่อนักเรียนไม่สำเร็จ'
		);
		applyStudents(result, ticket.revision);
	}

	function studentHref(studentId: string, suffix = ''): string {
		return withStudentRegisterContext(`/staff/students/${encodeURIComponent(studentId)}${suffix}`, {
			returnTo,
			academicYearId: null,
			source: 'registry'
		});
	}

	function changePage(nextPage: number) {
		if (nextPage < 1 || loading || deleting) return;
		const url = new URL(studentRegistryHref(page.url), page.url.origin);
		url.searchParams.set('page', String(nextPage));
		void goto(resolve(`staff/students?${url.searchParams.toString()}`));
	}

	function openDeleteDialog(student: StudentListItem) {
		if (!canDeleteStudent || deleting || student.account_status === 'inactive') return;
		studentToDelete = student;
		mutationError = '';
		showDeleteDialog = true;
	}

	async function confirmDelete() {
		if (
			!canDeleteStudent ||
			!studentToDelete ||
			deleting ||
			studentToDelete.account_status === 'inactive'
		)
			return;
		const id = studentToDelete.id,
			session = authStore.sessionEpoch,
			scope = readScope,
			key = listKey,
			epoch = ++mutationEpoch;
		const current = () =>
			!disposed &&
			session === authStore.sessionEpoch &&
			scope === readScope &&
			epoch === mutationEpoch &&
			key === listKey &&
			canDeleteStudent;
		studentsRequest.abort();
		loading = false;
		deleting = true;
		mutationError = '';
		try {
			await deleteStudent(id);
			if (!current()) return;
			toast.success('ปิดบัญชีนักเรียนสำเร็จ');
			if (studentToDelete?.id === id) {
				showDeleteDialog = false;
				studentToDelete = null;
			}
			await reloadRegistry();
		} catch (error) {
			if (current()) mutationError = error instanceof Error ? error.message : 'ปิดบัญชีไม่สำเร็จ';
		} finally {
			if (current()) deleting = false;
		}
	}
	function handleSearch() {
		if (!canReadStudents || deleting) return;
		const url = new URL(studentRegistryHref(page.url), page.url.origin);
		const search = searchQuery.trim();
		if (search) url.searchParams.set('search', search);
		else url.searchParams.delete('search');
		if (statusFilter === 'all') url.searchParams.delete('registrationStatus');
		else url.searchParams.set('registrationStatus', statusFilter);
		if (accountFilter === 'all') url.searchParams.delete('accountStatus');
		else url.searchParams.set('accountStatus', accountFilter);
		url.searchParams.delete('page');

		if (url.href === page.url.href) void reloadRegistry();
		else void goto(resolve(`staff/students?${url.searchParams.toString()}`));
	}
	function handleReset() {
		searchQuery = '';
		statusFilter = 'all';
		accountFilter = 'all';
		handleSearch();
	}
</script>

{#snippet studentName(student: StudentListItem)}
	{student.title || ''}{student.first_name} {student.last_name}
{/snippet}

{#snippet studentStatus(student: StudentListItem)}
	<Badge variant="secondary">{REGISTRATION_STATUS_LABELS[student.registration_status]}</Badge>
{/snippet}

<PageShell
	title="ทะเบียนนักเรียน"
	description="ทะเบียนกลางของนักเรียนทุกสถานะ การจัดชั้นและห้องอยู่ในทะเบียนรายปี"
>
	{#snippet actions()}
		{#if canCreateStudent}
			<Button href={createHref}><Plus class="size-4" aria-hidden="true" />เพิ่มนักเรียน</Button>
		{/if}
	{/snippet}

	{#if !canReadStudents}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูรายชื่อนักเรียน"
			description="บัญชีนี้ยังไม่มีสิทธิ์อ่านข้อมูลนักเรียนในขอบเขตที่ระบบอนุญาต"
		/>
	{:else}
		<form
			class="rounded-xl border bg-card p-3 sm:p-4"
			onsubmit={(event) => {
				event.preventDefault();
				handleSearch();
			}}
			aria-label="ค้นหาและกรองนักเรียน"
		>
			<div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_12rem_10rem_auto] lg:items-end">
				<div class="space-y-2">
					<Label for="student-search">ค้นหานักเรียน</Label>
					<div class="relative">
						<Search
							class="absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
							aria-hidden="true"
						/>
						<Input
							id="student-search"
							type="search"
							bind:value={searchQuery}
							placeholder="ค้นหาชื่อ หรือรหัสนักเรียน..."
							class="pl-9"
							disabled={deleting}
						/>
					</div>
				</div>
				<div class="space-y-2">
					<Label for="student-status-filter">สถานะทะเบียน</Label>
					<Select.Root
						type="single"
						value={statusFilter}
						onValueChange={(status) => {
							if (status && status !== statusFilter) {
								statusFilter = status;
								handleSearch();
							}
						}}
						disabled={deleting}
					>
						<Select.Trigger id="student-status-filter" class="w-full"
							>{statusFilter === 'all'
								? 'ทั้งหมด'
								: REGISTRATION_STATUS_LABELS[
										statusFilter as keyof typeof REGISTRATION_STATUS_LABELS
									]}</Select.Trigger
						>
						<Select.Content
							><Select.Item value="all">ทั้งหมด</Select.Item
							>{#each Object.entries(REGISTRATION_STATUS_LABELS) as [code, label] (code)}<Select.Item
									value={code}>{label}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
				</div>
				<div class="space-y-2">
					<Label for="student-account-filter">สถานะบัญชี</Label>
					<Select.Root
						type="single"
						value={accountFilter}
						onValueChange={(status) => {
							if (status && status !== accountFilter) {
								accountFilter = status;
								handleSearch();
							}
						}}
						disabled={deleting}
					>
						<Select.Trigger id="student-account-filter" class="w-full"
							>{accountFilter === 'all'
								? 'ทั้งหมด'
								: accountStatusLabel(accountFilter)}</Select.Trigger
						>
						<Select.Content
							><Select.Item value="all">ทั้งหมด</Select.Item
							>{#each Object.entries(STUDENT_ACCOUNT_STATUS_LABELS) as [code, label] (code)}<Select.Item
									value={code}>{label}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
				</div>
				<div class="flex gap-2">
					<Button type="submit" disabled={deleting}>ค้นหา</Button>
					<Button onclick={handleReset} variant="outline" disabled={deleting}>ล้างตัวกรอง</Button>
				</div>
			</div>
		</form>

		<section
			data-testid="student-list"
			aria-busy={loading}
			aria-label="ผลการค้นหานักเรียน"
			class="space-y-3"
		>
			{#if loadError && loaded}
				<PageState
					variant="error"
					title="อัปเดตรายชื่อนักเรียนไม่สำเร็จ"
					description={loadError}
					actionLabel="ลองอีกครั้ง"
					onaction={reloadRegistry}
				/>
			{/if}
			{#if loading && !loaded}
				<div role="status" aria-label="กำลังโหลดรายชื่อนักเรียน">
					<PageSkeleton variant="table" rows={6} columns={5} />
				</div>
			{:else if loadError && !loaded}
				<PageState
					variant="error"
					title="โหลดรายชื่อนักเรียนไม่สำเร็จ"
					description={loadError}
					actionLabel="ลองอีกครั้ง"
					onaction={reloadRegistry}
				/>
			{:else}
				<div class="overflow-hidden rounded-xl border bg-card">
					<div
						class="flex flex-wrap items-center justify-between gap-2 border-b bg-primary/5 px-3 py-3 sm:px-4"
					>
						<div class="flex items-center gap-2">
							<GraduationCap class="size-5 text-primary" aria-hidden="true" />
							<h2 class="font-medium">รายชื่อนักเรียน</h2>
							<Badge variant="secondary">{students.length} คนในหน้านี้</Badge>
						</div>
						<p class="text-xs text-muted-foreground">หน้า {currentPage}</p>
					</div>
					{#if students.length === 0}
						<PageState
							class="border-0 shadow-none"
							title={currentPage > 1
								? 'ไม่มีนักเรียนในหน้านี้'
								: hasAppliedFilters
									? 'ไม่พบนักเรียนที่ตรงกับตัวกรอง'
									: 'ยังไม่มีข้อมูลในทะเบียนนักเรียน'}
							description={currentPage > 1
								? 'ย้อนกลับไปหน้าก่อนหน้าเพื่อดูรายชื่อนักเรียน'
								: hasAppliedFilters
									? 'ลองเปลี่ยนคำค้นหาหรือล้างตัวกรองเพื่อดูรายชื่ออีกครั้ง'
									: 'เพิ่มข้อมูลนักเรียนเพื่อเริ่มทะเบียนกลาง'}
						>
							{#snippet action()}
								{#if currentPage > 1}<Button
										variant="outline"
										onclick={() => changePage(currentPage - 1)}
										disabled={loading || deleting}>← ก่อนหน้า</Button
									>
								{:else if hasAppliedFilters}<Button
										variant="outline"
										onclick={handleReset}
										disabled={deleting}>ล้างตัวกรอง</Button
									>
								{:else if canCreateStudent}<Button href={createHref}
										><Plus class="size-4" aria-hidden="true" />เพิ่มนักเรียน</Button
									>{/if}
							{/snippet}
						</PageState>
					{:else}
						<Table.Root class="table-fixed md:table-auto" aria-label="รายชื่อนักเรียน">
							<Table.Header
								><Table.Row class="bg-muted/40">
									<Table.Head class="hidden pl-4 md:table-cell">รหัสนักเรียน</Table.Head>
									<Table.Head class="pl-3 md:pl-2">ชื่อ-นามสกุล</Table.Head>
									<Table.Head class="hidden md:table-cell">วันที่เข้าเรียน</Table.Head>
									<Table.Head class="hidden md:table-cell">สถานะทะเบียน / บัญชี</Table.Head>
									<Table.Head class="w-28 pr-3 text-right md:w-auto md:pr-4">จัดการ</Table.Head>
								</Table.Row></Table.Header
							>
							<Table.Body>
								{#each students as student (student.id)}
									<Table.Row>
										<Table.Cell class="hidden pl-4 font-mono md:table-cell"
											>{student.student_id || 'ยังไม่มีรหัส'}</Table.Cell
										>
										<Table.Cell class="space-y-1 py-3 pl-3 whitespace-normal md:pl-2">
											<a
												href={studentHref(student.id)}
												data-sveltekit-preload-data="tap"
												class="break-words font-medium text-foreground hover:text-primary hover:underline"
												>{@render studentName(student)}</a
											>
											<div class="space-y-1 text-xs text-muted-foreground md:hidden">
												<p class="break-words">
													รหัส {student.student_id || 'ยังไม่มีรหัส'} · เข้าเรียน {student.enrollment_date
														? formatCalendarDate(student.enrollment_date)
														: '—'}
												</p>
												{@render studentStatus(student)}
												<span>บัญชี: {accountStatusLabel(student.account_status)}</span>
											</div>
										</Table.Cell>
										<Table.Cell class="hidden whitespace-normal md:table-cell"
											>{student.enrollment_date
												? formatCalendarDate(student.enrollment_date)
												: '—'}</Table.Cell
										>
										<Table.Cell class="hidden md:table-cell"
											><div class="space-y-1">
												{@render studentStatus(student)}
												<p class="text-xs text-muted-foreground">
													บัญชี: {accountStatusLabel(student.account_status)}
												</p>
											</div></Table.Cell
										>
										<Table.Cell class="pr-3 md:pr-4">
											<div class="flex justify-end gap-1">
												<Button
													href={studentHref(student.id)}
													aria-label="ดูข้อมูลนักเรียน"
													data-sveltekit-preload-data="tap"
													variant="ghost"
													size="icon-sm"><Eye class="size-4" aria-hidden="true" /></Button
												>
												{#if canUpdateStudent}<Button
														href={studentHref(student.id, '/edit')}
														aria-label="แก้ไขนักเรียน"
														data-sveltekit-preload-data="tap"
														variant="ghost"
														size="icon-sm"><Pencil class="size-4" aria-hidden="true" /></Button
													>{/if}
												{#if canDeleteStudent && student.account_status !== 'inactive'}<Button
														onclick={() => openDeleteDialog(student)}
														aria-label="ปิดบัญชีนักเรียน"
														disabled={deleting}
														variant="ghost"
														size="icon-sm"
														class="text-destructive hover:text-destructive"
														><Trash2 class="size-4" aria-hidden="true" /></Button
													>{/if}
											</div>
										</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					{/if}
					{#if students.length > 0}
						<div
							class="flex flex-wrap items-center justify-between gap-3 border-t px-3 py-3 sm:px-4"
						>
							<p class="text-xs text-muted-foreground">แสดง {students.length} รายการในหน้านี้</p>
							<nav aria-label="เปลี่ยนหน้ารายชื่อนักเรียน" class="flex items-center gap-2">
								<Button
									onclick={() => changePage(currentPage - 1)}
									disabled={currentPage === 1 || loading || deleting}
									variant="outline"
									size="sm">← ก่อนหน้า</Button
								>
								<span class="text-sm tabular-nums">หน้า {currentPage}</span>
								<Button
									onclick={() => changePage(currentPage + 1)}
									disabled={!hasNextPage || loading || deleting}
									variant="outline"
									size="sm">ถัดไป →</Button
								>
							</nav>
						</div>
					{/if}
				</div>
			{/if}
		</section>
	{/if}
</PageShell>

<!-- Delete Confirmation Dialog -->
{#if canDeleteStudent}
	<Dialog bind:open={showDeleteDialog}>
		<DialogContent>
			<DialogHeader>
				<DialogTitle>ยืนยันปิดบัญชีนักเรียน</DialogTitle>
				<DialogDescription>
					คุณแน่ใจหรือไม่ว่าต้องการปิดบัญชีนักเรียน
					{#if studentToDelete}
						<strong>
							{studentToDelete.title || ''}{studentToDelete.first_name}
							{studentToDelete.last_name}
						</strong>
					{/if}? บัญชีจะเข้าสู่ระบบไม่ได้และการเข้าร่วมปีการศึกษาที่ยังแก้ไขได้จะสิ้นสุด
					ข้อมูลและสถานะทะเบียนกลางยังคงอยู่
				</DialogDescription>
			</DialogHeader>
			{#if mutationError}<p role="alert" class="text-sm text-destructive">{mutationError}</p>{/if}
			<DialogFooter>
				<DialogCancel disabled={deleting}>ยกเลิก</DialogCancel>
				<Button variant="destructive" onclick={confirmDelete} disabled={deleting} class="gap-2">
					<Trash2 class="h-4 w-4" />
					{deleting ? 'กำลังปิดบัญชี...' : 'ปิดบัญชี'}
				</Button>
			</DialogFooter>
		</DialogContent>
	</Dialog>
{/if}
