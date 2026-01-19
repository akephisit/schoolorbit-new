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
	import { GraduationCap, Plus, Search, Pencil, Trash2, Eye } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let students: StudentListItem[] = $state([]);
	let loading = $state(true);
	let deleting = $state(false);
	let showDeleteDialog = $state(false);
	let studentToDelete: StudentListItem | null = $state(null);
	let searchQuery = $state('');
	let gradeFilter = $state('');
	let classFilter = $state('');
	let currentPage = $state(1);
	let totalPages = $state(1);
	let total = $state(0);

	function formatFullClassRoom(name: string) {
		if (!name) return '-';
		if (name.startsWith('อ.')) return name.replace('อ.', 'อนุบาลปีที่ ');
		if (name.startsWith('ป.')) return name.replace('ป.', 'ประถมศึกษาปีที่ ');
		if (name.startsWith('ม.')) return name.replace('ม.', 'มัธยมศึกษาปีที่ ');
		return name;
	}

	async function loadStudents() {
		try {
			loading = true;
			const response = await listStudents({
				search: searchQuery || undefined,
				grade_level: gradeFilter || undefined,
				class_room: classFilter || undefined,
				page: currentPage,
				page_size: 20
			});

			students = response.data;
			total = response.total || 0;
			totalPages = response.total_pages || 1;
		} catch (e) {
			const message = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
			console.error('Failed to load students:', e);
		} finally {
			loading = false;
		}
	}

	function openDeleteDialog(student: StudentListItem) {
		studentToDelete = student;
		showDeleteDialog = true;
	}

	async function confirmDelete() {
		if (!studentToDelete) return;

		deleting = true;
		try {
			await deleteStudent(studentToDelete.id);
			toast.success('ลบนักเรียนสำเร็จ');
			showDeleteDialog = false;
			studentToDelete = null;
			await loadStudents();
		} catch (e) {
			const message = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			deleting = false;
		}
	}

	function handleSearch() {
		currentPage = 1;
		loadStudents();
	}

	function handleReset() {
		searchQuery = '';
		gradeFilter = '';
		classFilter = '';
		currentPage = 1;
		loadStudents();
	}

	onMount(() => {
		loadStudents();
	});
</script>

<svelte:head>
	<title>จัดการนักเรียน - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
		<div>
			<h1 class="text-3xl font-bold text-foreground flex items-center gap-2">
				<GraduationCap class="w-8 h-8" />
				จัดการนักเรียน
			</h1>
			<p class="text-muted-foreground mt-1">จัดการข้อมูลนักเรียนทั้งหมด</p>
		</div>
		<Button href="/staff/students/new" class="flex items-center gap-2">
			<Plus class="w-4 h-4" />
			เพิ่มนักเรียน
		</Button>
	</div>

	<!-- Search and Filter -->
	<div class="bg-card border border-border rounded-lg p-4">
		<div class="grid grid-cols-1 md:grid-cols-4 gap-4">
			<div class="md:col-span-2 relative">
				<Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
				<Input
					type="text"
					bind:value={searchQuery}
					onkeypress={(e) => e.key === 'Enter' && handleSearch()}
					placeholder="ค้นหาชื่อ หรือรหัสนักเรียน..."
					class="pl-10"
				/>
			</div>

			<Input type="text" bind:value={gradeFilter} placeholder="ระดับชั้น (เช่น ม.1)" />

			<Input type="text" bind:value={classFilter} placeholder="ห้อง (เช่น 1, 2)" />
		</div>

		<div class="flex gap-2 mt-4">
			<Button onclick={handleSearch}>ค้นหา</Button>
			<Button onclick={handleReset} variant="outline">ล้างตัวกรอง</Button>
		</div>
	</div>

	<!-- Student List -->
	{#if loading}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<div
				class="inline-block w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"
			></div>
			<p class="mt-4 text-muted-foreground">กำลังโหลด...</p>
		</div>
	{:else if students.length === 0}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<GraduationCap class="w-16 h-16 mx-auto text-muted-foreground mb-4" />
			<p class="text-lg font-medium text-foreground">ไม่พบนักเรียน</p>
			<p class="text-muted-foreground mt-2">
				{searchQuery || gradeFilter || classFilter
					? 'ไม่พบนักเรียนที่ตรงกับเงื่อนไขที่ค้นหา'
					: 'เริ่มต้นด้วยการเพิ่มนักเรียนคนแรก'}
			</p>
			{#if !searchQuery && !gradeFilter && !classFilter}
				<Button href="/staff/students/new" class="mt-4">
					<Plus class="w-4 h-4 mr-2" />
					เพิ่มนักเรียน
				</Button>
			{/if}
		</div>
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
								{#if student.class_room}
									<span class="text-sm md:hidden">{student.class_room}</span>
									<span class="hidden md:inline text-sm"
										>{formatFullClassRoom(student.class_room)}</span
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
								<Button href="/staff/students/{student.id}" variant="ghost" size="sm">
									<Eye class="w-4 h-4" />
								</Button>
								<Button href="/staff/students/{student.id}/edit" variant="ghost" size="sm">
									<Pencil class="w-4 h-4" />
								</Button>
								<Button onclick={() => openDeleteDialog(student)} variant="ghost" size="sm">
									<Trash2 class="h-4 w-4" />
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
							แสดง {students.length} จาก {total} รายการ
						</p>
						<div class="flex gap-2">
							<Button
								onclick={() => {
									currentPage--;
									loadStudents();
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
									loadStudents();
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

<!-- Delete Confirmation Dialog -->
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
