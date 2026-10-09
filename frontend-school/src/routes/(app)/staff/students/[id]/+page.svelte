<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import type { PageProps } from './$types';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Card } from '#lib/components/ui/card/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import StudentProfileHistory from '#lib/components/academic-core/StudentProfileHistory.svelte';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import { Edit } from '@lucide/svelte';
	import { getStudent, type Student } from '#lib/api/students.js';

	let { data }: PageProps = $props();
	const studentId = $derived(data.studentId);
	const profileKey = $derived(data.profileKey);
	const source = $derived(data.student);
	const studentRequest = new LatestRequest();
	let activeKey = '';
	let student = $state.raw<Student | null>(null);
	let loading = $state(true);
	let error = $state('');
	const academicYearId = $derived(data.academicYearId);
	const academicYearQuery = $derived(
		academicYearId ? `?academicYearId=${encodeURIComponent(academicYearId)}` : ''
	);
	const listHref = $derived(data.returnTo);
	const editHref = $derived(
		`/staff/students/${encodeURIComponent(studentId)}/edit${academicYearQuery}`
	);

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

	$effect.pre(() => {
		const key = profileKey,
			read = source;
		untrack(() => {
			if (key !== activeKey) {
				activeKey = key;
				student = null;
			}
			const ticket = studentRequest.begin();
			loading = true;
			error = '';
			void read.then((result) => applyStudent(result, ticket.revision));
		});
		return () => studentRequest.abort();
	});

	function applyStudent(result: Awaited<typeof data.student>, revision: number) {
		if (!studentRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			student = result.data;
		} else error = result.error;
	}
	async function reloadCurrentYear() {
		if (!academicYearId || !canReadStudent) return;
		const ticket = studentRequest.begin();
		loading = true;
		error = '';
		const result = await captureRouteLoad(
			getStudent(studentId, academicYearId, { signal: ticket.signal }),
			'โหลดข้อมูลนักเรียนไม่สำเร็จ'
		);
		applyStudent(result, ticket.revision);
	}
</script>

<PageShell
	title={student && canReadStudent
		? `${student.title ?? ''}${student.first_name} ${student.last_name}`
		: 'นักเรียน'}
	description="รายละเอียดข้อมูลนักเรียน"
	backHref={listHref}
	backPreload={false}
>
	{#snippet actions()}
		<Button variant="outline" onclick={reloadCurrentYear} disabled={loading || !canReadStudent}
			>รีเฟรช</Button
		>
		{#if student && canReadStudent && canUpdateStudent}
			<Button href={editHref} data-sveltekit-preload-data="tap">
				<Edit class="w-4 h-4 mr-2" />
				แก้ไข
			</Button>
		{/if}
	{/snippet}

	<section data-testid="student-profile" aria-busy={loading}>
		{#if error && student && canReadStudent}<PageState
				title="อัปเดตข้อมูลนักเรียนไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={reloadCurrentYear}
			/>{/if}
		{#if loading && student && canReadStudent}<p role="status">กำลังอัปเดตข้อมูลนักเรียน...</p>{/if}
		{#if !canReadStudent}
			<PageState
				variant="permission"
				title="ไม่มีสิทธิ์ดูข้อมูลนักเรียน"
				description="บัญชีนี้ยังไม่มีสิทธิ์อ่านข้อมูลนักเรียนคนนี้ในขอบเขตที่ระบบอนุญาต"
			/>
		{:else if loading && !student}
			<div role="status" aria-label="กำลังโหลดข้อมูลนักเรียน">
				<PageSkeleton variant="detail" />
			</div>
		{:else if error && !student}
			<PageState
				variant="error"
				title="โหลดข้อมูลนักเรียนไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={reloadCurrentYear}
			/>
		{:else if student}
			<!-- Student ID & Status -->
			<Card class="p-6">
				<div class="flex items-center justify-between gap-4">
					{#if student.profile_image_file_id}<PrivateFileImage
							fileId={student.profile_image_file_id}
							resourceId={student.id}
							alt={`รูป ${student.first_name}`}
							class="size-20 rounded-xl object-cover bg-muted"
						/>{/if}
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

				<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 sm:gap-6">
					<div>
						<Label>ชื่อ-นามสกุล</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{`${student.title ?? ''}${student.first_name}`}
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

					<div class="sm:col-span-2">
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

				<div class="grid grid-cols-1 gap-4 sm:grid-cols-3 sm:gap-6">
					<div>
						<Label>ระดับชั้น</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.grade_level || '-'}
						</div>
					</div>

					<div>
						<Label>ห้อง</Label>
						<div class="px-3 py-2 bg-muted/50 rounded-md">
							{student.homeroom || '-'}
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

					<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 sm:gap-6">
						{#if student.blood_type}
							<div>
								<Label>หมู่เลือด</Label>
								<div class="px-3 py-2 bg-muted/50 rounded-md">
									{student.blood_type}
								</div>
							</div>
						{/if}

						{#if student.allergies}
							<div class="sm:col-span-2">
								<Label>อาการแพ้</Label>
								<div class="px-3 py-2 bg-muted/50 rounded-md">
									{student.allergies}
								</div>
							</div>
						{/if}

						{#if student.medical_conditions}
							<div class="sm:col-span-2">
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
				href={listHref}
			/>
		{/if}
	</section>
	{#if canReadStudent}<StudentProfileHistory
			source={data.history}
			{profileKey}
			{studentId}
			{academicYearId}
		/>{/if}
</PageShell>
