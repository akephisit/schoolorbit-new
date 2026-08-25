<script lang="ts">
	import type { PageProps } from './$types';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		listChildAcademicContextOptions,
		type AcademicContextOptionsResponse
	} from '$lib/api/academic-context';
	import {
		resolveScopedAcademicYearUrl,
		urlWithAcademicYear
	} from '$lib/academic-context/scoped-year';
	import { getChildProfile } from '$lib/api/parents';
	import type { Student } from '$lib/api/students';
	import ScopedAcademicYearSelect from '$lib/components/academic-context/ScopedAcademicYearSelect.svelte';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Label } from '$lib/components/ui/label';
	import { User, Calendar, BookOpen, Clock } from 'lucide-svelte';
	import { formatDate } from '$lib/utils/date';
	import PrivateFileImage from '$lib/components/files/PrivateFileImage.svelte';

	let { params }: PageProps = $props();
	let studentId = $derived(params.id);
	let contextOptions = $state<AcademicContextOptionsResponse | null>(null);
	let selectedYearId = $state('');
	let student = $state<Student | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let revision = 0;

	const academicYearQuery = $derived(
		selectedYearId ? `?academicYearId=${encodeURIComponent(selectedYearId)}` : ''
	);
	const parentHref = $derived(`${resolve('/parent')}${academicYearQuery}`);
	const timetableHref = $derived(
		`${resolve(`/parent/student/${studentId}/timetable`)}${academicYearQuery}`
	);

	async function loadStudent() {
		const current = ++revision;
		if (!selectedYearId) {
			student = null;
			loading = false;
			return;
		}
		loading = true;
		error = null;
		try {
			const loaded = await getChildProfile(studentId, selectedYearId);
			if (current !== revision) return;
			student = loaded;
		} catch (e) {
			if (current !== revision) return;
			console.error('Failed to load student:', e);
			error = e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลได้';
		} finally {
			if (current === revision) loading = false;
		}
	}

	async function initialize(): Promise<void> {
		const current = ++revision;
		loading = true;
		error = null;
		try {
			const options = await listChildAcademicContextOptions(studentId);
			if (current !== revision) return;
			contextOptions = options;
			const selection = resolveScopedAcademicYearUrl(options, page.url);
			selectedYearId = selection.academicYearId ?? '';

			if (selection.replaceUrl) {
				await goto(
					resolve(
						`/parent/student/${studentId}${selection.replaceUrl.search}${selection.replaceUrl.hash}`
					),
					{ replaceState: true, noScroll: true, keepFocus: true }
				);
				if (current !== revision) return;
			}

			if (!selectedYearId) {
				student = null;
				loading = false;
				return;
			}
			await loadStudent();
		} catch (loadError) {
			if (current !== revision) return;
			error = loadError instanceof Error ? loadError.message : 'โหลดประวัติปีการศึกษาไม่สำเร็จ';
			loading = false;
		}
	}

	async function changeAcademicYear(academicYearId: string): Promise<void> {
		if (academicYearId === selectedYearId) return;
		const current = ++revision;
		selectedYearId = academicYearId;
		loading = true;
		error = null;
		try {
			const nextUrl = urlWithAcademicYear(page.url, academicYearId);
			await goto(resolve(`/parent/student/${studentId}${nextUrl.search}${nextUrl.hash}`), {
				replaceState: true,
				noScroll: true,
				keepFocus: true
			});
			if (current !== revision) return;
			await loadStudent();
		} catch (loadError) {
			if (current !== revision) return;
			error = loadError instanceof Error ? loadError.message : 'เปลี่ยนปีการศึกษาไม่สำเร็จ';
			loading = false;
		}
	}

	function retry(): void {
		if (contextOptions && selectedYearId) void loadStudent();
		else void initialize();
	}

	onMount(() => {
		void initialize();
	});
</script>

<PageShell
	title={student
		? `${student.title || ''}${student.first_name} ${student.last_name}`
		: 'ข้อมูลนักเรียน'}
	description={student
		? `${student.grade_level || 'ไม่ระบุชั้น'} | ห้อง ${student.homeroom || '-'} | รหัสนักเรียน: ${student.student_number || '-'}`
		: 'ข้อมูลนักเรียนที่เชื่อมโยงกับบัญชีผู้ปกครอง'}
	backHref={parentHref}
>
	{#if contextOptions && contextOptions.years.length > 0}
		<div class="flex max-w-sm flex-col gap-2 rounded-xl border bg-card p-4">
			<Label for="parent-child-detail-year">ปีการศึกษา</Label>
			<ScopedAcademicYearSelect
				id="parent-child-detail-year"
				years={contextOptions.years}
				value={selectedYearId}
				disabled={loading}
				onchange={changeAcademicYear}
			/>
		</div>
	{/if}

	{#if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดข้อมูลนักเรียนไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={retry}
		/>
	{:else if contextOptions && contextOptions.years.length === 0}
		<PageState
			title="ยังไม่มีประวัติปีการศึกษาสำหรับนักเรียนคนนี้"
			description="กรุณาติดต่อโรงเรียนเพื่อตรวจสอบการลงทะเบียน"
		/>
	{:else if student}
		<!-- Header -->
		<div class="flex flex-col md:flex-row gap-6 items-start">
			<div
				class="w-32 h-32 rounded-full bg-muted flex items-center justify-center overflow-hidden border-4 border-background shadow-lg"
			>
				{#if student.profile_image_file_id}
					<PrivateFileImage
						fileId={student.profile_image_file_id}
						resourceId={student.id}
						alt={student.first_name}
						class="w-full h-full object-cover"
					/>
				{:else}
					<User class="w-12 h-12 text-muted-foreground/50" />
				{/if}
			</div>

			<div class="flex-1">
				<div class="flex flex-wrap gap-2 mb-4">
					<Badge variant="secondary" class="text-sm px-3 py-1">
						{student.grade_level || 'ไม่ระบุชั้น'}
					</Badge>
					<Badge variant="outline" class="text-sm px-3 py-1 text-muted-foreground">
						ห้อง {student.homeroom || '-'}
					</Badge>
					<Badge variant="outline" class="text-sm px-3 py-1 text-muted-foreground">
						รหัสนักเรียน: {student.student_number || '-'}
					</Badge>
				</div>

				<div class="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm text-muted-foreground">
					<div class="flex items-center gap-2">
						<Calendar class="w-4 h-4" />
						วันเกิด: {student.date_of_birth ? formatDate(student.date_of_birth) : '-'}
					</div>
				</div>
			</div>
		</div>

		<!-- Content Tabs -->
		<!-- Placeholder for future features like Grades, Timetable, Attendance -->
		<div class="grid grid-cols-1 md:grid-cols-3 gap-6 mt-8">
			<Card class="p-6">
				<div class="flex items-center gap-3 mb-4">
					<div
						class="p-2 rounded-lg bg-blue-100 text-blue-600 dark:bg-blue-900/30 dark:text-blue-400"
					>
						<Clock class="w-5 h-5" />
					</div>
					<h3 class="font-semibold">การเข้าเรียน</h3>
				</div>
				<p class="text-muted-foreground text-sm">ยังไม่มีข้อมูลการเข้าเรียน</p>
				<Button variant="link" class="px-0 mt-2 text-blue-600">ดูทั้งหมด</Button>
			</Card>

			<Card class="p-6">
				<div class="flex items-center gap-3 mb-4">
					<div
						class="p-2 rounded-lg bg-green-100 text-green-600 dark:bg-green-900/30 dark:text-green-400"
					>
						<BookOpen class="w-5 h-5" />
					</div>
					<h3 class="font-semibold">ผลการเรียน</h3>
				</div>
				<p class="text-muted-foreground text-sm">ยังไม่มีข้อมูลผลการเรียน</p>
				<Button variant="link" class="px-0 mt-2 text-green-600">ดูทั้งหมด</Button>
			</Card>

			<Card class="p-6">
				<div class="flex items-center gap-3 mb-4">
					<div
						class="p-2 rounded-lg bg-purple-100 text-purple-600 dark:bg-purple-900/30 dark:text-purple-400"
					>
						<Calendar class="w-5 h-5" />
					</div>
					<h3 class="font-semibold">ตารางเรียน</h3>
				</div>
				<p class="text-muted-foreground text-sm">ดูตารางเรียนของบุตรในแต่ละภาคเรียน</p>
				<Button variant="link" class="px-0 mt-2 text-purple-600" href={timetableHref}>
					ดูทั้งหมด
				</Button>
			</Card>
		</div>
	{:else}
		<PageState
			title="ไม่พบข้อมูลนักเรียน"
			description="ไม่พบข้อมูลนักเรียนที่เชื่อมโยงกับบัญชีผู้ปกครองนี้"
			actionLabel="กลับหน้าผู้ปกครอง"
			href={parentHref}
		/>
	{/if}
</PageShell>
