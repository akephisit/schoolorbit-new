<script lang="ts">
	import { Table, TableBody, TableRow, TableCell } from '#lib/components/ui/table/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import MathContent from '#lib/components/question-bank/MathContent.svelte';
	import ExamSessionBlock from '#lib/components/academic/exam-schedule/ExamSessionBlock.svelte';
	import type { ExamSession } from '#lib/api/examSchedule.js';
	import { Toaster, toast } from 'svelte-sonner';
	import { Card } from '#lib/components/ui/card/index.js';

	const id = '61000000-0000-4000-8000-000000000001';
	const session: ExamSession = {
		id,
		academicTermId: id,
		academicYearId: id,
		assessmentPhaseId: id,
		courseAssessmentPlanId: id,
		durationMinutes: 60,
		examDayId: id,
		examRoundId: id,
		examScheduleItemId: id,
		gradeLevelId: id,
		homeroomId: id,
		learningGroupId: id,
		learningOfferingId: id,
		subjectId: id,
		importedAt: '2026-01-01T00:00:00Z',
		invigilators: [],
		startsAt: '09:00:00',
		endsAt: '10:00:00',
		subjectNameTh: 'คณิตศาสตร์'
	};
</script>

<Toaster position="bottom-right" />
<main>
	<div class="relative h-14" data-testid="exam-time">
		<ExamSessionBlock {session} leftPx={0} widthPx={200} readonly />
	</div>
	<div class="relative h-14" data-testid="exam-status">
		<ExamSessionBlock {session} leftPx={0} widthPx={200} placing />
	</div>
	<Button
		onclick={() =>
			toast('ข้อความแจ้งเตือน SchoolOrbit', {
				description: 'รายละเอียดทดสอบ 123',
				action: { label: 'รับทราบ', onClick: () => {} },
				duration: Infinity
			})}>แสดงข้อความแจ้งเตือน</Button
	>
	<code class="font-mono" data-testid="technical-code">const total = 123;</code>
	<MathContent latex="x^2 + 1" />
	<p class="font-sans" data-testid="font-sans-consumer">แบบอักษรของโรงเรียน</p>
	<Button data-testid="primary-button">บันทึก</Button>
	<Button data-testid="destructive-button" variant="destructive">ลบ</Button>
	<Badge data-testid="destructive-badge" variant="destructive" href="#example">เกินกำหนด</Badge>
	<Table
		><TableBody
			><TableRow
				><TableCell data-testid="default-cell">ข้อมูลตัวอย่าง</TableCell><TableCell
					data-testid="explicit-cell"
					class="py-3">กำหนดระยะเฉพาะ</TableCell
				></TableRow
			></TableBody
		></Table
	>
	<Card data-testid="explicit-card" class="gap-0 py-0"
		><p>หนึ่ง</p>
		<p>สอง</p></Card
	>
</main>
