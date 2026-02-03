<script lang="ts">
	import { page as pageState } from '$app/state';
	import { onMount } from 'svelte';
	import { getChildProfile } from '$lib/api/parents';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Tabs } from '$lib/components/ui/tabs';
	import { ArrowLeft, User, Calendar, BookOpen, Clock } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { formatDate } from '$lib/utils/date';

	let studentId = $derived(pageState.params.id as string);
	let student = $state<any>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	onMount(async () => {
		try {
			const response = await getChildProfile(studentId);
			student = response.data;
		} catch (e) {
			console.error('Failed to load student:', e);
			error = e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลได้';
		} finally {
			loading = false;
		}
	});
</script>

<svelte:head>
	<title>ข้อมูลนักเรียน - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<Button variant="ghost" onclick={() => goto('/parent')} class="pl-0 gap-2">
		<ArrowLeft class="w-4 h-4" /> ย้อนกลับ
	</Button>

	{#if loading}
		<div class="p-8 text-center text-muted-foreground">กำลังโหลดข้อมูล...</div>
	{:else if error}
		<div class="p-8 text-center text-destructive bg-destructive/10 rounded-lg">
			<p>{error}</p>
		</div>
	{:else if student}
		<!-- Header -->
		<div class="flex flex-col md:flex-row gap-6 items-start">
			<div
				class="w-32 h-32 rounded-full bg-muted flex items-center justify-center overflow-hidden border-4 border-background shadow-lg"
			>
				{#if student.info.profile_image_url}
					<img
						src={student.info.profile_image_url}
						alt={student.info.first_name}
						class="w-full h-full object-cover"
					/>
				{:else}
					<User class="w-12 h-12 text-muted-foreground/50" />
				{/if}
			</div>

			<div class="flex-1">
				<h1 class="text-3xl font-bold tracking-tight mb-2">
					{student.info.title || ''}{student.info.first_name}
					{student.info.last_name}
				</h1>
				<div class="flex flex-wrap gap-2 mb-4">
					<Badge variant="secondary" class="text-sm px-3 py-1">
						{student.info.grade_level || 'ไม่ระบุชั้น'}
					</Badge>
					<Badge variant="outline" class="text-sm px-3 py-1 text-muted-foreground">
						ห้อง {student.info.class_room || '-'}
					</Badge>
					<Badge variant="outline" class="text-sm px-3 py-1 text-muted-foreground">
						รหัสนักเรียน: {student.info.student_number || '-'}
					</Badge>
				</div>

				<div class="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm text-muted-foreground">
					<div class="flex items-center gap-2">
						<Calendar class="w-4 h-4" />
						วันเกิด: {student.info.date_of_birth ? formatDate(student.info.date_of_birth) : '-'}
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
				<p class="text-muted-foreground text-sm">ยังไม่มีข้อมูลตารางเรียน</p>
				<Button variant="link" class="px-0 mt-2 text-purple-600">ดูทั้งหมด</Button>
			</Card>
		</div>
	{/if}
</div>
