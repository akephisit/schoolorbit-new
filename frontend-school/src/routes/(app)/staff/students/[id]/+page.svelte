<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { resolve } from '$app/paths';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Edit } from 'lucide-svelte';
	import { getStudent } from '$lib/api/students';

	let studentId = $derived($page.params.id as string);
	let student = $state<any>(null);
	let loading = $state(true);

	onMount(async () => {
		await loadStudent();
	});

	async function loadStudent() {
		loading = true;
		try {
			const response = await getStudent(studentId);
			student = response.data;
		} catch (error) {
			console.error('Failed to load student:', error);
			const message = error instanceof Error ? error.message : 'ไม่พบนักเรียน';
			toast.error(message);
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>
		{student ? `${student.first_name} ${student.last_name}` : 'นักเรียน'} - SchoolOrbit
	</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-4">
			<Button href="/staff/students" variant="ghost" size="sm">
				<ArrowLeft class="w-4 h-4" />
			</Button>
			<div>
				<h1 class="text-2xl font-bold text-foreground">
					{#if student}
						{student.first_name} {student.last_name}
					{:else}
						นักเรียน
					{/if}
				</h1>
				<p class="text-sm text-muted-foreground">รายละเอียดข้อมูลนักเรียน</p>
			</div>
		</div>

		{#if student}
			<Button href="/staff/students/{studentId}/edit">
				<Edit class="w-4 h-4 mr-2" />
				แก้ไข
			</Button>
		{/if}
	</div>

	{#if loading}
		<Card class="p-6">
			<div class="space-y-4">
				{#each Array(6) as _}
					<div class="animate-pulse">
						<div class="h-4 bg-muted rounded w-1/4 mb-2"></div>
						<div class="h-10 bg-muted rounded"></div>
					</div>
				{/each}
			</div>
		</Card>
	{:else if student}
		<!-- Student ID & Status -->
		<Card class="p-6">
			<div class="flex items-center justify-between">
				<div>
					<p class="text-sm text-muted-foreground">รหัสนักเรียน</p>
					<p class="text-2xl font-bold">{student.student_id}</p>
				</div>
				<Badge
					variant={student.status === 'active' ? 'default' : 'secondary'}
					class={student.status === 'active' ? 'bg-green-500' : ''}
				>
					{student.status === 'active' ? 'ใช้งาน' : 'ไม่ใช้งาน'}
				</Badge>
			</div>
		</Card>

		<!-- Basic Information -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-6">ข้อมูลพื้นฐาน</h2>

			<div class="grid grid-cols-2 gap-6">
				<div>
					<Label>ชื่อ-นามสกุล</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.title || ''}
						{student.first_name}
						{student.last_name}
					</div>
				</div>

				<div>
					<Label>เพศ</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.gender === 'male' ? 'ชาย' : student.gender === 'female' ? 'หญิง' : '-'}
					</div>
				</div>

				<div>
					<Label>วันเกิด</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.date_of_birth || '-'}
					</div>
				</div>

				<div>
					<Label>เลขบัตรประชาชน</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.national_id || '-'}
					</div>
				</div>

				<div>
					<Label>อีเมล</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.email || '-'}
					</div>
				</div>

				<div>
					<Label>เบอร์โทรศัพท์</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.phone || '-'}
					</div>
				</div>

				<div class="col-span-2">
					<Label>ที่อยู่</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.address || '-'}
					</div>
				</div>
			</div>
		</Card>

		<!-- Student Information -->
		<Card class="p-6">
			<h2 class="text-xl font-semibold mb-6">ข้อมูลนักเรียน</h2>

			<div class="grid grid-cols-3 gap-6">
				<div>
					<Label>ระดับชั้น</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.grade_level || '-'}
					</div>
				</div>

				<div>
					<Label>ห้อง</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.class_room || '-'}
					</div>
				</div>

				<div>
					<Label>เลขที่</Label>
					<div class="px-3 py-2 bg-muted/50 rounded-md">
						{student.student_number || '-'}
					</div>
				</div>
			</div>
		</Card>

		<!-- Medical Information (if any) -->
		{#if student.blood_type || student.allergies || student.medical_conditions}
			<Card class="p-6">
				<h2 class="text-xl font-semibold mb-6">ข้อมูลสุขภาพ</h2>

				<div class="grid grid-cols-2 gap-6">
					{#if student.blood_type}
						<div>
							<Label>หมู่เลือด</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md">
								{student.blood_type}
							</div>
						</div>
					{/if}

					{#if student.allergies}
						<div class="col-span-2">
							<Label>อาการแพ้</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md">
								{student.allergies}
							</div>
						</div>
					{/if}

					{#if student.medical_conditions}
						<div class="col-span-2">
							<Label>โรคประจำตัว</Label>
							<div class="px-3 py-2 bg-muted/50 rounded-md">
								{student.medical_conditions}
							</div>
						</div>
					{/if}
				</div>
			</Card>
		{/if}
	{/if}
</div>
