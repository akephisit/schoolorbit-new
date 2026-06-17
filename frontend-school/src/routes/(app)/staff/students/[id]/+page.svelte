<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { Label } from '$lib/components/ui/label';
	import { Card } from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { toast } from 'svelte-sonner';
	import { Edit } from 'lucide-svelte';
	import { getStudent, type Student } from '$lib/api/students';

	let { params }: PageProps = $props();
	let studentId = $derived(params.id);
	let student = $state<Student | null>(null);
	let loading = $state(true);
	let error = $state('');

	const canReadStudent = $derived(
		$can.hasAny(
			PERMISSIONS.STUDENT_READ_SCHOOL,
			PERMISSIONS.STUDENT_READ_ASSIGNED,
			PERMISSIONS.STUDENT_READ_OWN
		)
	);
	const canUpdateStudent = $derived($can.has(PERMISSIONS.STUDENT_UPDATE_ALL));
	const canReadStudentPii = $derived(
		$can.hasAny(
			PERMISSIONS.STUDENT_PII_READ_SCHOOL,
			PERMISSIONS.STUDENT_PII_READ_ASSIGNED,
			PERMISSIONS.STUDENT_PII_READ_OWN
		)
	);

	onMount(async () => {
		await loadStudent();
	});

	async function loadStudent() {
		if (!canReadStudent) {
			student = null;
			loading = false;
			return;
		}
		loading = true;
		error = '';
		try {
			const response = await getStudent(studentId);
			student = response.data;
		} catch (loadError) {
			console.error('Failed to load student:', loadError);
			const message = loadError instanceof Error ? loadError.message : 'ไม่พบนักเรียน';
			error = message;
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

<PageShell
	title={student ? `${student.first_name} ${student.last_name}` : 'นักเรียน'}
	description="รายละเอียดข้อมูลนักเรียน"
	backHref="/staff/students"
>
	{#snippet actions()}
		{#if student && canUpdateStudent}
			<Button href="/staff/students/{studentId}/edit">
				<Edit class="w-4 h-4 mr-2" />
				แก้ไข
			</Button>
		{/if}
	{/snippet}

	{#if !canReadStudent}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูข้อมูลนักเรียน"
			description="บัญชีนี้ยังไม่มีสิทธิ์อ่านข้อมูลนักเรียนคนนี้ในขอบเขตที่ระบบอนุญาต"
		/>
	{:else if loading}
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

				{#if canReadStudentPii}
					<div>
						<Label>เลขบัตรประชาชน</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.national_id || '-'}
						</div>
					</div>
				{:else}
					<div>
						<Label>เลขบัตรประชาชน</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md text-muted-foreground">
							ไม่มีสิทธิ์ดูข้อมูลส่วนบุคคล
						</div>
					</div>
				{/if}

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
	{:else}
		<PageState
			title="ไม่พบนักเรียน"
			description="ข้อมูลนักเรียนนี้อาจถูกลบหรือคุณอาจไม่มีสิทธิ์เข้าถึง"
			actionLabel="กลับหน้ารายชื่อนักเรียน"
			href="/staff/students"
		/>
	{/if}
</PageShell>
