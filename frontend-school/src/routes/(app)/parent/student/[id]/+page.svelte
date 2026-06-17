<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import { getChildProfile } from '$lib/api/parents';
	import type { Student } from '$lib/api/students';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { ArrowLeft, User, Calendar, BookOpen, Clock } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { formatDate } from '$lib/utils/date';

	let { params }: PageProps = $props();
	let studentId = $derived(params.id);
	let student = $state<Student | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function loadStudent() {
		loading = true;
		error = null;
		try {
			const response = await getChildProfile(studentId);
			student = response.data;
		} catch (e) {
			console.error('Failed to load student:', e);
			error = e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลได้';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		loadStudent();
	});
</script>

<svelte:head>
	<title>ข้อมูลนักเรียน - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<Button variant="ghost" onclick={() => goto(resolve('/parent'))} class="pl-0 gap-2">
		<ArrowLeft class="w-4 h-4" /> ย้อนกลับ
	</Button>

	{#if loading}
		<PageSkeleton variant="detail" />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดข้อมูลนักเรียนไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadStudent}
		/>
	{:else if student}
		<!-- Header -->
		<div class="flex flex-col md:flex-row gap-6 items-start">
			<div
				class="w-32 h-32 rounded-full bg-muted flex items-center justify-center overflow-hidden border-4 border-background shadow-lg"
			>
				{#if student.profile_image_url}
					<img
						src={student.profile_image_url}
						alt={student.first_name}
						class="w-full h-full object-cover"
					/>
				{:else}
					<User class="w-12 h-12 text-muted-foreground/50" />
				{/if}
			</div>

			<div class="flex-1">
				<h1 class="text-3xl font-bold tracking-tight mb-2">
					{student.title || ''}{student.first_name}
					{student.last_name}
				</h1>
				<div class="flex flex-wrap gap-2 mb-4">
					<Badge variant="secondary" class="text-sm px-3 py-1">
						{student.grade_level || 'ไม่ระบุชั้น'}
					</Badge>
					<Badge variant="outline" class="text-sm px-3 py-1 text-muted-foreground">
						ห้อง {student.class_room || '-'}
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
				<Button
					variant="link"
					class="px-0 mt-2 text-purple-600"
					onclick={() =>
						// eslint-disable-next-line @typescript-eslint/no-explicit-any -- dynamic typed-route interpolation
						goto(resolve(`/parent/student/${studentId}/timetable` as any))}
				>
					ดูทั้งหมด
				</Button>
			</Card>
		</div>
	{:else}
		<PageState
			title="ไม่พบข้อมูลนักเรียน"
			description="ไม่พบข้อมูลนักเรียนที่เชื่อมโยงกับบัญชีผู้ปกครองนี้"
			actionLabel="กลับหน้าผู้ปกครอง"
			href="/parent"
		/>
	{/if}
</div>
