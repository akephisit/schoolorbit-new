<script lang="ts">
	import { onMount } from 'svelte';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { User, Calendar, BookOpen, Award } from 'lucide-svelte';
	import { authAPI } from '$lib/api/auth';

	let student = $state<any>(null);
	let loading = $state(true);

	onMount(async () => {
		try {
			const response = await fetch('/api/student/profile', {
				headers: {
					Authorization: `Bearer ${localStorage.getItem('auth_token')}`
				}
			});

			if (response.ok) {
				const data = await response.json();
				student = data.data;
			}
		} catch (error) {
			console.error('Failed to load profile:', error);
		} finally {
			loading = false;
		}
	});
</script>

<svelte:head>
	<title>แดชบอร์ด - Student Portal</title>
</svelte:head>

<div class="container mx-auto p-6 space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold text-foreground">แดชบอร์ด</h1>
			{#if student}
				<p class="text-muted-foreground mt-1">
					สวัสดี, {student.first_name}
					{student.last_name}
				</p>
			{/if}
		</div>
	</div>

	{#if loading}
		<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
			{#each Array(3) as _}
				<Card class="p-6 animate-pulse">
					<div class="h-4 bg-muted rounded w-1/2 mb-4"></div>
					<div class="h-8 bg-muted rounded w-3/4"></div>
				</Card>
			{/each}
		</div>
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
							{#if student.grade_level && student.class_room}
								{student.grade_level}/{student.class_room}
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
						<p class="text-2xl font-bold text-green-600">95%</p>
					</div>
					<div class="w-12 h-12 bg-green-500/10 rounded-lg flex items-center justify-center">
						<Calendar class="w-6 h-6 text-green-500" />
					</div>
				</div>
			</Card>
		</div>

		<!-- Quick Actions -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-4">เมนูด่วน</h2>
			<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
				<Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/student/profile">
					<User class="w-6 h-6" />
					<span>ข้อมูลส่วนตัว</span>
				</Button>

				<Button variant="outline" class="h-auto py-4 flex-col gap-2" disabled>
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
	{/if}
</div>
