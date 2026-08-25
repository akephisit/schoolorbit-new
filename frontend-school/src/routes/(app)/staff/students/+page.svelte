<script lang="ts">
	import { onMount } from 'svelte';
	import { listStudents, deleteStudent, type StudentListItem } from '$lib/api/students';
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
	import * as Select from '$lib/components/ui/select';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Plus, Search, Pencil, Trash2, Eye } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

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
	const academicContext = getAcademicContextStore();
	const academicYearId = $derived($academicContext.selected.academicYearId);
	const PAGE_SIZE = 20;

	let students = $state.raw<StudentListItem[]>([]);
	let loading = $state(true);
	let deleting = $state(false);
	let showDeleteDialog = $state(false);
	let studentToDelete: StudentListItem | null = $state(null);
	let searchQuery = $state('');

	let statusFilter = $state('active');
	let currentPage = $state(1);
	let hasNextPage = $state(false);
	let revision = 0;

	function formatFullClassRoom(name: string, gradeLevel?: string | null) {
		if (!name) return '-';

		// If name has prefix/format
		if (
			name.startsWith('อ.') ||
			name.startsWith('ป.') ||
			name.startsWith('ม.') ||
			name.includes('/')
		) {
			if (name.startsWith('อ.')) return name.replace('อ.', 'อนุบาลปีที่ ');
			if (name.startsWith('ป.')) return name.replace('ป.', 'ประถมศึกษาปีที่ ');
			if (name.startsWith('ม.')) return name.replace('ม.', 'มัธยมศึกษาปีที่ ');
			return name;
		}

		// If just number/code, prepend grade
		if (gradeLevel) {
			let fullGrade = gradeLevel;
			if (gradeLevel.startsWith('อ.')) fullGrade = gradeLevel.replace('อ.', 'อนุบาลปีที่ ');
			else if (gradeLevel.startsWith('ป.'))
				fullGrade = gradeLevel.replace('ป.', 'ประถมศึกษาปีที่ ');
			else if (gradeLevel.startsWith('ม.'))
				fullGrade = gradeLevel.replace('ม.', 'มัธยมศึกษาปีที่ ');
			return `${fullGrade}/${name}`;
		}

		return name;
	}

	async function loadStudents(yearId: string) {
		const current = ++revision;
		if (!canReadStudents) {
			if (current === revision) {
				students = [];
				hasNextPage = false;
				loading = false;
			}
			return;
		}
		try {
			loading = true;
			const result = await listStudents({
				academicYearId: yearId,
				search: searchQuery || undefined,
				status: statusFilter === 'all' ? undefined : statusFilter,
				page: currentPage,
				pageSize: PAGE_SIZE
			});

			if (current !== revision) return;
			students = result.items;
			currentPage = result.page;
			hasNextPage = result.items.length === result.page_size;
		} catch (e) {
			if (current !== revision) return;
			const message = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
			console.error('Failed to load students:', e);
		} finally {
			if (current === revision) loading = false;
		}
	}

	function reloadCurrentYear() {
		if (!academicYearId) return;
		void loadStudents(academicYearId);
	}

	function studentHref(studentId: string, suffix = ''): string {
		const path = `/staff/students/${encodeURIComponent(studentId)}${suffix}`;
		return academicYearId ? `${path}?academicYearId=${encodeURIComponent(academicYearId)}` : path;
	}

	function changePage(nextPage: number) {
		if (!academicYearId || nextPage < 1) return;
		currentPage = nextPage;
		void loadStudents(academicYearId);
	}

	function openDeleteDialog(student: StudentListItem) {
		if (!canDeleteStudent) return;
		studentToDelete = student;
		showDeleteDialog = true;
	}

	async function confirmDelete() {
		if (!canDeleteStudent) return;
		if (!studentToDelete) return;

		deleting = true;
		try {
			await deleteStudent(studentToDelete.id);
			toast.success('ลบนักเรียนสำเร็จ');
			showDeleteDialog = false;
			studentToDelete = null;
			reloadCurrentYear();
		} catch (e) {
			const message = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			deleting = false;
		}
	}

	function handleSearch() {
		if (!canReadStudents) return;
		currentPage = 1;
		reloadCurrentYear();
	}

	function handleReset() {
		if (!canReadStudents) return;
		searchQuery = '';
		statusFilter = 'active';
		currentPage = 1;
		reloadCurrentYear();
	}

	onMount(() => {
		let loadedYearId = '';
		return academicContext.subscribe((state) => {
			const yearId = state.selected.academicYearId;
			if (!yearId || yearId === loadedYearId) return;
			loadedYearId = yearId;
			currentPage = 1;
			void loadStudents(yearId);
		});
	});
</script>

<PageShell title="จัดการนักเรียน" description="จัดการข้อมูลนักเรียนทั้งหมด">
	{#snippet actions()}
		{#if canCreateStudent}
			<Button href="/staff/students/new" class="flex items-center gap-2">
				<Plus class="w-4 h-4" />
				เพิ่มนักเรียน
			</Button>
		{/if}
	{/snippet}

	{#if !canReadStudents}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูรายชื่อนักเรียน"
			description="บัญชีนี้เข้า module นักเรียนได้ แต่ยังไม่มีสิทธิ์อ่านข้อมูลนักเรียนในขอบเขตที่ระบบอนุญาต"
		/>
	{:else}
		<!-- Search and Filter -->
		<div class="rounded-xl border bg-card p-3 sm:p-4">
			<div class="grid gap-3 md:grid-cols-[minmax(0,1fr)_minmax(12rem,16rem)_auto]">
				<div class="relative">
					<Search class="absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
					<Input
						type="text"
						bind:value={searchQuery}
						onkeypress={(e) => e.key === 'Enter' && handleSearch()}
						placeholder="ค้นหาชื่อ หรือรหัสนักเรียน..."
						class="pl-9"
					/>
				</div>

				<div>
					<Select.Root type="single" bind:value={statusFilter} onValueChange={handleSearch}>
						<Select.Trigger class="w-full">
							{statusFilter === 'active'
								? 'ใช้งาน (Active)'
								: statusFilter === 'inactive'
									? 'ไม่ใช้งาน (Inactive)'
									: 'ทั้งหมด'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="active">ใช้งาน (Active)</Select.Item>
							<Select.Item value="inactive">ไม่ใช้งาน (Inactive)</Select.Item>
							<Select.Item value="all">ทั้งหมด</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>

				<div class="flex gap-2 md:justify-end">
					<Button class="flex-1 md:flex-none" onclick={handleSearch}>ค้นหา</Button>
					<Button class="flex-1 md:flex-none" onclick={handleReset} variant="outline">
						ล้างตัวกรอง
					</Button>
				</div>
			</div>
		</div>

		<!-- Student List -->
		{#if loading}
			<PageSkeleton variant="table" rows={6} columns={5} />
		{:else if students.length === 0}
			<PageState
				title="ไม่พบนักเรียน"
				description={searchQuery
					? 'ไม่พบนักเรียนที่ตรงกับเงื่อนไขที่ค้นหา'
					: 'เริ่มต้นด้วยการเพิ่มนักเรียนคนแรก'}
				actionLabel={!searchQuery && canCreateStudent ? 'เพิ่มนักเรียน' : undefined}
				href={!searchQuery && canCreateStudent ? '/staff/students/new' : undefined}
			/>
		{:else}
			<div class="bg-card border border-border rounded-lg overflow-hidden">
				<!-- Table Header -->
				<div class="bg-muted/50 px-6 py-3 border-b border-border">
					<div class="grid grid-cols-12 gap-4 text-sm font-medium text-muted-foreground">
						<div class="col-span-2">รหัสนักเรียน</div>
						<div class="col-span-4">ชื่อ-นามสกุล</div>
						<div class="col-span-2">ชั้น</div>
						<div class="col-span-2">สถานะ</div>
						<div class="col-span-2 text-right">จัดการ</div>
					</div>
				</div>

				<!-- Table Body -->
				<div class="divide-y divide-border">
					{#each students as student (student.id)}
						<div class="px-6 py-4 hover:bg-accent/50 transition-colors">
							<div class="grid grid-cols-12 gap-4 items-center">
								<!-- Student ID -->
								<div class="col-span-2">
									<p class="font-mono text-sm">{student.student_id || '-'}</p>
								</div>

								<!-- Name -->
								<div class="col-span-4">
									<p class="font-medium text-foreground">
										{student.title || ''}{student.first_name}
										{student.last_name}
									</p>
								</div>

								<!-- Grade/Class -->
								<div class="col-span-2">
									{#if student.homeroom}
										<span class="text-sm md:hidden">
											{#if student.homeroom.includes('/') || student.homeroom.startsWith('อ.') || student.homeroom.startsWith('ป.') || student.homeroom.startsWith('ม.')}
												{student.homeroom}
											{:else}
												{student.grade_level}/{student.homeroom}
											{/if}
										</span>
										<span class="hidden md:inline text-sm"
											>{formatFullClassRoom(student.homeroom, student.grade_level)}</span
										>
									{:else}
										<span class="text-sm text-muted-foreground">-</span>
									{/if}
								</div>

								<!-- Status -->
								<div class="col-span-2">
									{#if student.status === 'active'}
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
									<Button href={studentHref(student.id)} variant="ghost" size="sm">
										<Eye class="w-4 h-4" />
									</Button>
									{#if canUpdateStudent}
										<Button href={studentHref(student.id, '/edit')} variant="ghost" size="sm">
											<Pencil class="w-4 h-4" />
										</Button>
									{/if}
									{#if canDeleteStudent}
										<Button onclick={() => openDeleteDialog(student)} variant="ghost" size="sm">
											<Trash2 class="h-4 w-4" />
										</Button>
									{/if}
								</div>
							</div>
						</div>
					{/each}
				</div>

				<!-- Pagination -->
				{#if currentPage > 1 || hasNextPage}
					<div class="bg-muted/30 px-6 py-4 border-t border-border">
						<div class="flex items-center justify-between">
							<p class="text-sm text-muted-foreground">
								แสดง {students.length} รายการในหน้านี้
							</p>
							<div class="flex gap-2">
								<Button
									onclick={() => changePage(currentPage - 1)}
									disabled={currentPage === 1}
									variant="outline"
									size="sm"
								>
									← ก่อนหน้า
								</Button>
								<span class="px-4 py-2 text-sm">
									หน้า {currentPage}
								</span>
								<Button
									onclick={() => changePage(currentPage + 1)}
									disabled={!hasNextPage}
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
	{/if}
</PageShell>

<!-- Delete Confirmation Dialog -->
{#if canDeleteStudent}
	<Dialog bind:open={showDeleteDialog}>
		<DialogContent>
			<DialogHeader>
				<DialogTitle>ยืนยันการลบนักเรียน</DialogTitle>
				<DialogDescription>
					คุณแน่ใจหรือไม่ว่าต้องการลบนักเรียน
					{#if studentToDelete}
						<strong>
							{studentToDelete.title || ''}{studentToDelete.first_name}
							{studentToDelete.last_name}
						</strong>
					{/if}? การกระทำนี้จะทำให้นักเรียนถูกปิดการใช้งาน
				</DialogDescription>
			</DialogHeader>
			<DialogFooter>
				<Button variant="outline" onclick={() => (showDeleteDialog = false)} disabled={deleting}>
					ยกเลิก
				</Button>
				<Button variant="destructive" onclick={confirmDelete} disabled={deleting} class="gap-2">
					<Trash2 class="h-4 w-4" />
					{deleting ? 'กำลังลบ...' : 'ลบนักเรียน'}
				</Button>
			</DialogFooter>
		</DialogContent>
	</Dialog>
{/if}
