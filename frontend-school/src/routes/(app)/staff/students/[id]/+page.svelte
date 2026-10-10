<script lang="ts">
	import { untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import type { PageProps } from './$types';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import StudentProfileHistory from '#lib/components/academic-core/StudentProfileHistory.svelte';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import { Edit, GraduationCap, HeartPulse, RefreshCw, UserRound } from '@lucide/svelte';
	import { getStudent, type Student } from '#lib/api/students.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';

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
		<Button variant="outline" onclick={reloadCurrentYear} disabled={loading || !canReadStudent}>
			<RefreshCw class={loading ? 'size-4 animate-spin' : 'size-4'} aria-hidden="true" />
			รีเฟรช
		</Button>
		{#if student && canReadStudent && canUpdateStudent}
			<Button href={editHref} data-sveltekit-preload-data="tap">
				<Edit class="size-4" aria-hidden="true" />
				แก้ไข
			</Button>
		{/if}
	{/snippet}

	<section class="space-y-6" data-testid="student-profile" aria-busy={loading}>
		{#if error && student && canReadStudent}<PageState
				variant="error"
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
			<Card class="overflow-hidden" data-testid="student-summary">
				<CardContent>
					<div class="flex flex-col gap-5 sm:flex-row sm:items-center">
						{#if student.profile_image_file_id}
							<PrivateFileImage
								fileId={student.profile_image_file_id}
								resourceId={student.id}
								alt={`รูป ${student.first_name}`}
								class="size-20 shrink-0 rounded-2xl bg-muted object-cover"
							/>
						{:else}
							<div
								class="flex size-20 shrink-0 items-center justify-center rounded-2xl bg-primary/10 text-primary"
								aria-hidden="true"
							>
								<UserRound class="size-9" />
							</div>
						{/if}
						<div class="min-w-0 flex-1 space-y-3">
							<div class="flex flex-wrap items-center gap-2">
								<p class="text-sm text-muted-foreground">ข้อมูลนักเรียน</p>
								<Badge variant={student.status === 'active' ? 'default' : 'secondary'}>
									{student.status === 'active' ? 'ใช้งาน' : 'ไม่ใช้งาน'}
								</Badge>
							</div>
							<p class="break-words text-xl font-semibold sm:text-2xl">
								{student.title ?? ''}{student.first_name}
								{student.last_name}
							</p>
							<p class="break-words text-sm text-muted-foreground">
								รหัสนักเรียน <span class="font-medium text-foreground"
									>{student.student_id || '—'}</span
								>
							</p>
						</div>
						<div class="flex min-w-0 items-center gap-3 rounded-xl bg-muted/50 p-4 sm:max-w-xs">
							<GraduationCap class="size-5 shrink-0 text-muted-foreground" aria-hidden="true" />
							<div class="min-w-0">
								<p class="break-words text-sm font-medium">
									{student.grade_level || 'ยังไม่ระบุระดับชั้น'}
								</p>
								<p class="break-words text-sm text-muted-foreground">
									{student.homeroom || 'ยังไม่ระบุห้อง'}
								</p>
							</div>
						</div>
					</div>
				</CardContent>
			</Card>

			<div class="grid items-start gap-6 xl:grid-cols-2">
				<div class="min-w-0 space-y-6">
					<Card data-testid="student-basic">
						<CardHeader>
							<CardTitle><h2>ข้อมูลพื้นฐาน</h2></CardTitle>
							<CardDescription>ข้อมูลส่วนตัวและการระบุตัวตน</CardDescription>
						</CardHeader>
						<CardContent>
							<dl class="grid gap-6 sm:grid-cols-2">
								<div class="min-w-0 space-y-1.5 sm:col-span-2">
									<dt class="text-sm text-muted-foreground">ชื่อ-นามสกุล</dt>
									<dd class="break-words font-medium">
										{student.title ?? ''}{student.first_name}
										{student.last_name}
									</dd>
								</div>
								<div class="space-y-1.5">
									<dt class="text-sm text-muted-foreground">เพศ</dt>
									<dd>
										{student.gender === 'male' ? 'ชาย' : student.gender === 'female' ? 'หญิง' : '—'}
									</dd>
								</div>
								<div class="space-y-1.5">
									<dt class="text-sm text-muted-foreground">วันเกิด</dt>
									<dd>{student.date_of_birth ? formatCalendarDate(student.date_of_birth) : '—'}</dd>
								</div>
								<div class="min-w-0 space-y-1.5 sm:col-span-2">
									<dt class="text-sm text-muted-foreground">เลขบัตรประชาชน</dt>
									<dd class="break-words" class:text-muted-foreground={!canReadStudentPii}>
										{canReadStudentPii
											? student.national_id || '—'
											: 'ไม่มีสิทธิ์ดูข้อมูลส่วนบุคคล'}
									</dd>
								</div>
							</dl>
						</CardContent>
					</Card>

					<Card data-testid="student-contact">
						<CardHeader>
							<CardTitle><h2>ข้อมูลติดต่อ</h2></CardTitle>
							<CardDescription>ช่องทางติดต่อและที่อยู่ของนักเรียน</CardDescription>
						</CardHeader>
						<CardContent>
							<dl class="grid gap-6 sm:grid-cols-2">
								<div class="min-w-0 space-y-1.5 sm:col-span-2">
									<dt class="text-sm text-muted-foreground">อีเมล</dt>
									<dd class="break-words">{student.email || '—'}</dd>
								</div>
								<div class="space-y-1.5 sm:col-span-2">
									<dt class="text-sm text-muted-foreground">เบอร์โทรศัพท์</dt>
									<dd>{student.phone || '—'}</dd>
								</div>
								<div class="min-w-0 space-y-1.5 sm:col-span-2">
									<dt class="text-sm text-muted-foreground">ที่อยู่</dt>
									<dd class="break-words whitespace-pre-line">{student.address || '—'}</dd>
								</div>
							</dl>
						</CardContent>
					</Card>
				</div>

				<div class="min-w-0 space-y-6">
					<Card data-testid="student-academic">
						<CardHeader>
							<CardTitle><h2>ข้อมูลการเรียน</h2></CardTitle>
							<CardDescription>ระดับชั้นและห้องในปีการศึกษาที่เลือก</CardDescription>
						</CardHeader>
						<CardContent>
							<dl class="grid gap-6 sm:grid-cols-3">
								<div class="min-w-0 space-y-1.5">
									<dt class="text-sm text-muted-foreground">ระดับชั้น</dt>
									<dd class="break-words font-medium">{student.grade_level || '—'}</dd>
								</div>
								<div class="min-w-0 space-y-1.5">
									<dt class="text-sm text-muted-foreground">ห้อง</dt>
									<dd class="break-words font-medium">{student.homeroom || '—'}</dd>
								</div>
								<div class="space-y-1.5">
									<dt class="text-sm text-muted-foreground">เลขที่</dt>
									<dd class="font-medium">{student.student_number ?? '—'}</dd>
								</div>
							</dl>
						</CardContent>
					</Card>

					{#if student.blood_type || student.allergies || student.medical_conditions}
						<Card data-testid="student-health">
							<CardHeader>
								<CardTitle
									><h2 class="flex items-center gap-2">
										<HeartPulse
											class="size-4 text-muted-foreground"
											aria-hidden="true"
										/>ข้อมูลสุขภาพ
									</h2></CardTitle
								>
								<CardDescription>ข้อมูลประกอบการดูแลนักเรียน</CardDescription>
							</CardHeader>
							<CardContent>
								<dl class="space-y-6">
									{#if student.blood_type}
										<div class="space-y-1.5">
											<dt class="text-sm text-muted-foreground">หมู่เลือด</dt>
											<dd>{student.blood_type}</dd>
										</div>
									{/if}
									{#if student.allergies}
										<div class="space-y-1.5">
											<dt class="text-sm text-muted-foreground">อาการแพ้</dt>
											<dd class="break-words whitespace-pre-line">{student.allergies}</dd>
										</div>
									{/if}
									{#if student.medical_conditions}
										<div class="space-y-1.5">
											<dt class="text-sm text-muted-foreground">โรคประจำตัว</dt>
											<dd class="break-words whitespace-pre-line">{student.medical_conditions}</dd>
										</div>
									{/if}
								</dl>
							</CardContent>
						</Card>
					{/if}
				</div>
			</div>
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
