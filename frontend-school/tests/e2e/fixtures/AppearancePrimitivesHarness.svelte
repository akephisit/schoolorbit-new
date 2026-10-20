<script lang="ts">
	import { Table, TableBody, TableRow, TableCell } from '#lib/components/ui/table/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import MathContent from '#lib/components/question-bank/MathContent.svelte';
	import ExamSessionBlock from '#lib/components/academic/exam-schedule/ExamSessionBlock.svelte';
	import type { ExamSession } from '#lib/api/examSchedule.js';
	import { Toaster, toast } from 'svelte-sonner';
	import { Check } from '@lucide/svelte';
	import { Card } from '#lib/components/ui/card/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { ToggleGroup, ToggleGroupItem } from '#lib/components/ui/toggle-group/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	let pressed = $state(false);

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
	<Button data-testid="primary-button"><Check data-testid="primary-button-icon" />บันทึก</Button>
	<Badge data-testid="primary-badge" href="#example">สีโรงเรียน</Badge>
	<Button variant="link" data-testid="primary-link">ลิงก์โรงเรียน</Button>
	<Check class="text-primary" data-testid="primary-icon" />
	<Card><p class="text-primary" data-testid="primary-card-text">ข้อความบนการ์ด</p></Card>
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

	<section class="mx-auto max-w-3xl space-y-4 p-4" aria-label="ตัวอย่างเครื่องมือร่วม">
		<Card class="gap-4 p-4" data-testid="controls-card">
			<h2>แบบฟอร์มและตัวเลือก</h2>
			<Input aria-label="ชื่อรายการ" placeholder="กรอกชื่อรายการ" data-testid="surface-input" />
			<Textarea
				aria-label="รายละเอียด"
				placeholder="กรอกรายละเอียด"
				data-testid="surface-textarea"
			/>
			<Select.Root type="single" value="first">
				<Select.Trigger aria-label="ประเภท" data-testid="surface-select">ประเภทแรก</Select.Trigger>
				<Select.Content
					><Select.Item value="first">ประเภทแรก</Select.Item><Select.Item value="second"
						>ประเภทที่สอง</Select.Item
					></Select.Content
				>
			</Select.Root>
			<div class="flex flex-wrap gap-2">
				<Button variant="outline" data-testid="surface-outline">ยกเลิก</Button>
				<Button
					variant="secondary"
					aria-pressed={pressed}
					onclick={() => (pressed = !pressed)}
					data-testid="surface-secondary">เลือกกลุ่ม</Button
				>
				<Button
					variant="outline"
					aria-pressed={pressed}
					class="border-primary/40 bg-accent"
					onclick={() => (pressed = !pressed)}
					data-testid="surface-pressed">เลือกรอบ</Button
				>
				<Button variant="secondary" aria-current="page" data-testid="surface-current"
					>หน้าปัจจุบัน</Button
				>
				<Button disabled variant="outline" data-testid="surface-disabled">ยังไม่พร้อม</Button>
			</div>
			<Tabs.Root value="first">
				<Tabs.List aria-label="มุมมอง"
					><Tabs.Trigger value="first" data-testid="surface-tab">รายการ</Tabs.Trigger><Tabs.Trigger
						value="second">สรุป</Tabs.Trigger
					></Tabs.List
				>
				<Tabs.Content value="first">ข้อมูลรายการ</Tabs.Content><Tabs.Content value="second"
					>ข้อมูลสรุป</Tabs.Content
				>
			</Tabs.Root>
			<ToggleGroup type="single" value="first" aria-label="รูปแบบรายการ">
				<ToggleGroupItem value="first" data-testid="surface-toggle">ตาราง</ToggleGroupItem
				><ToggleGroupItem value="second">การ์ด</ToggleGroupItem>
			</ToggleGroup>
			<div class="flex flex-wrap items-center gap-4">
				<Checkbox aria-label="เลือกรายการ" data-testid="surface-checkbox" />
				<RadioGroup.Root value="first" aria-label="ตัวเลือก"
					><RadioGroup.Item value="first" data-testid="surface-radio" /></RadioGroup.Root
				>
				<Switch aria-label="เปิดใช้งาน" data-testid="surface-switch" />
			</div>
			<p class="text-muted-foreground" data-testid="surface-muted-text">
				ข้อความประกอบที่อ่านได้ชัดเจน
			</p>
		</Card>
		<div class="bg-muted rounded-lg p-4" data-testid="controls-tray">
			<Input aria-label="ค้นหาในรายการ" placeholder="ค้นหา" data-testid="tray-input" />
		</div>
	</section>
</main>
