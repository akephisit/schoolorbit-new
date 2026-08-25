<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		listMyAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Label } from '$lib/components/ui/label';
	import ScopedAcademicYearSelect from '$lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import {
		resolveScopedAcademicYearUrl,
		urlWithAcademicYear
	} from '$lib/academic-context/scoped-year';
	import { User, Calendar, BookOpen, Award } from 'lucide-svelte';
	import { getOwnProfile, type Student } from '$lib/api/students';
	import { toast } from 'svelte-sonner';

	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let student = $state<Student | null>(null);
	let loading = $state(true);
	let error = $state('');
	let revision = 0;

	const academicYearQuery = $derived(
		selectedYearId ? `?academicYearId=${encodeURIComponent(selectedYearId)}` : ''
	);
	const profileHref = $derived(`${resolve('/student/profile')}${academicYearQuery}`);
	const timetableHref = $derived(`${resolve('/student/timetable')}${academicYearQuery}`);

	async function loadProfile() {
		const current = ++revision;
		if (!selectedYearId) {
			student = null;
			loading = false;
			return;
		}
		loading = true;
		error = '';
		try {
			const loaded = await getOwnProfile(selectedYearId);
			if (current !== revision) return;
			student = loaded;
		} catch (loadError) {
			if (current !== revision) return;
			console.error('Failed to load profile:', loadError);
			const message = loadError instanceof Error ? loadError.message : 'เกิดข้อผิดพลาด';
			error = message;
			toast.error(message);
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function initialize(): Promise<void> {
		const current = ++revision;
		loading = true;
		error = '';
		try {
			const options = await listMyAcademicContextOptions();
			if (current !== revision) return;
			contextOptions = options;
			const selection = resolveScopedAcademicYearUrl(options, page.url);
			selectedYearId = selection.academicYearId ?? '';

			if (selection.replaceUrl) {
				await goto(resolve(`/student${selection.replaceUrl.search}${selection.replaceUrl.hash}`), {
					replaceState: true,
					noScroll: true,
					keepFocus: true
				});
				if (current !== revision) return;
			}

			if (!selectedYearId) {
				student = null;
				loading = false;
				return;
			}
			await loadProfile();
		} catch (loadError) {
			if (current !== revision) return;
			const message =
				loadError instanceof Error ? loadError.message : 'โหลดประวัติปีการศึกษาไม่สำเร็จ';
			error = message;
			toast.error(message);
			loading = false;
		}
	}

	async function changeAcademicYear(academicYearId: string): Promise<void> {
		if (academicYearId === selectedYearId) return;
		const current = ++revision;
		selectedYearId = academicYearId;
		loading = true;
		error = '';
		try {
			const nextUrl = urlWithAcademicYear(page.url, academicYearId);
			await goto(resolve(`/student${nextUrl.search}${nextUrl.hash}`), {
				replaceState: true,
				noScroll: true,
				keepFocus: true
			});
			if (current !== revision) return;
			await loadProfile();
		} catch (loadError) {
			if (current !== revision) return;
			const message = loadError instanceof Error ? loadError.message : 'เปลี่ยนปีการศึกษาไม่สำเร็จ';
			error = message;
			toast.error(message);
			loading = false;
		}
	}

	function retry(): void {
		if (contextOptions && selectedYearId) void loadProfile();
		else void initialize();
	}

	onMount(() => {
		void initialize();
	});
</script>

<PageShell
	title="แดชบอร์ด"
	description={student
		? `สวัสดี, ${student.first_name} ${student.last_name}`
		: 'ภาพรวมข้อมูลนักเรียน'}
>
	{#if contextOptions && contextOptions.years.length > 0}
		<div class="flex max-w-sm flex-col gap-2 rounded-xl border bg-card p-4">
			<Label for="student-dashboard-year">ปีการศึกษา</Label>
			<ScopedAcademicYearSelect
				id="student-dashboard-year"
				years={contextOptions.years}
				value={selectedYearId}
				disabled={loading}
				onchange={changeAcademicYear}
			/>
		</div>
	{/if}

	{#if loading}
		<PageSkeleton variant="cards" rows={3} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดแดชบอร์ดไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={retry}
		/>
	{:else if contextOptions && contextOptions.years.length === 0}
		<PageState
			title="ยังไม่มีประวัติปีการศึกษาสำหรับบัญชีนี้"
			description="กรุณาติดต่อผู้ดูแลระบบเพื่อตรวจสอบการลงทะเบียนนักเรียน"
		/>
	{:else if student}
		<!-- Student Info Cards -->
		<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
			<!-- Student ID Card -->
			<Card class="p-6 hover:shadow-md transition-shadow">
				<div class="flex items-start justify-between">
					<div class="space-y-2">
						<p class="text-sm text-muted-foreground font-medium">รหัสนักเรียน</p>
						<p class="text-2xl font-bold text-foreground">
							{student.student_id || '-'}
						</p>
					</div>
					<div class="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center">
						<User class="w-6 h-6 text-primary" />
					</div>
				</div>
			</Card>

			<!-- Class Card -->
			<Card class="p-6 hover:shadow-md transition-shadow">
				<div class="flex items-start justify-between">
					<div class="space-y-2">
						<p class="text-sm text-muted-foreground font-medium">ชั้นเรียน</p>
						<p class="text-2xl font-bold text-foreground">
							{#if student.grade_level && student.homeroom}
								{student.grade_level}/{student.homeroom}
							{:else}
								-
							{/if}
						</p>
					</div>
					<div class="w-12 h-12 bg-blue-500/10 rounded-lg flex items-center justify-center">
						<BookOpen class="w-6 h-6 text-blue-500" />
					</div>
				</div>
			</Card>

			<!-- Attendance Card -->
			<Card class="p-6 hover:shadow-md transition-shadow">
				<div class="flex items-start justify-between">
					<div class="space-y-2">
						<p class="text-sm text-muted-foreground font-medium">การเข้าเรียน</p>
						<p class="text-2xl font-bold text-muted-foreground">—</p>
						<p class="text-xs text-muted-foreground">ยังไม่เปิดใช้งาน</p>
					</div>
					<div class="w-12 h-12 bg-muted rounded-lg flex items-center justify-center">
						<Calendar class="w-6 h-6 text-muted-foreground" />
					</div>
				</div>
			</Card>
		</div>

		<!-- Quick Actions -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-4">เมนูด่วน</h2>
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
				<Button variant="outline" class="h-auto py-4 flex-col gap-2" href={profileHref}>
					<User class="w-6 h-6" />
					<span>ข้อมูลส่วนตัว</span>
				</Button>

				<Button variant="outline" class="h-auto py-4 flex-col gap-2" href={timetableHref}>
					<BookOpen class="w-6 h-6" />
					<span>ตารางเรียน</span>
				</Button>

				<Button variant="outline" class="h-auto py-4 flex-col gap-2" disabled>
					<Award class="w-6 h-6" />
					<span>คะแนน</span>
				</Button>

				<Button variant="outline" class="h-auto py-4 flex-col gap-2" disabled>
					<Calendar class="w-6 h-6" />
					<span>การเข้าเรียน</span>
				</Button>
			</div>
			<p class="text-sm text-muted-foreground mt-4 text-center">
				เมนูที่เป็นสีเทาจะเปิดใช้งานในอนาคต
			</p>
		</Card>

		<!-- Announcements (placeholder) -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-4">ประกาศ</h2>
			<div class="text-center py-8 text-muted-foreground">
				<p>ไม่มีประกาศในขณะนี้</p>
			</div>
		</Card>
	{:else}
		<PageState
			title="ไม่พบข้อมูลนักเรียน"
			description="ไม่พบโปรไฟล์นักเรียนของบัญชีนี้ กรุณาติดต่อผู้ดูแลระบบ"
		/>
	{/if}
</PageShell>
