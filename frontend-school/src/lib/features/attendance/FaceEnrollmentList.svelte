<script lang="ts">
	import {
		Card,
		CardHeader,
		CardTitle,
		CardDescription,
		CardContent
	} from '#lib/components/ui/card/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	import {
		currentAttendanceDate,
		type AttendanceFaceEnrollment,
		type AttendanceStudentOption
	} from '#lib/api/attendance.js';
	let {
		students,
		enrollments,
		disabled,
		onremove,
		onrefresh
	}: {
		students: AttendanceStudentOption[];
		enrollments: AttendanceFaceEnrollment[];
		disabled: boolean;
		onremove: (id: string) => void;
		onrefresh: () => void;
	} = $props();
	let search = $state('');
	let page = $state(1);
	const studentById = $derived(new Map(students.map((student) => [student.id, student])));
	const filtered = $derived(
		enrollments.flatMap((enrollment) => {
			const student = studentById.get(enrollment.studentId);
			return student &&
				`${student.name} ${student.homeroomName ?? ''}`
					.toLocaleLowerCase()
					.includes(search.trim().toLocaleLowerCase())
				? [{ ...enrollment, student }]
				: [];
		})
	);
	const pages = $derived(Math.max(1, Math.ceil(filtered.length / 40)));
	const currentPage = $derived(Math.min(page, pages));
	const rows = $derived(filtered.slice((currentPage - 1) * 40, currentPage * 40));
</script>

<Card class="min-w-0" aria-label="รายชื่อที่ลงทะเบียนใบหน้า">
	<CardHeader
		><CardTitle><h2>รายชื่อที่ลงทะเบียนใบหน้า</h2></CardTitle><CardDescription
			>ดูสถานะและถอนข้อมูลใบหน้าที่ได้รับอนุญาต ({enrollments.length} คน)</CardDescription
		></CardHeader
	>
	<CardContent class="space-y-4">
		<div class="flex flex-wrap items-end gap-3">
			<div class="min-w-0 flex-1 space-y-2">
				<Label for="attendance-face-search">ค้นหาชื่อหรือห้องเรียน</Label><Input
					id="attendance-face-search"
					bind:value={search}
					oninput={() => (page = 1)}
				/>
			</div>
			<Button type="button" variant="outline" onclick={onrefresh} {disabled}>โหลดรายชื่อใหม่</Button
			>
		</div>
		{#if !rows.length}<PageState
				variant="empty"
				title={enrollments.length ? 'ไม่พบรายชื่อที่ค้นหา' : 'ยังไม่มีผู้ลงทะเบียนใบหน้า'}
				description={enrollments.length
					? 'ลองค้นหาด้วยชื่อหรือห้องเรียนอื่น'
					: 'เลือกนักเรียนและเก็บตัวอย่างใบหน้าด้านบน'}
			/>
		{:else}<ul class="divide-y rounded-lg border">
				{#each rows as row (row.studentId)}<li
						class="flex flex-wrap items-center justify-between gap-3 p-3"
					>
						<div class="min-w-0 space-y-1">
							<p class="break-words font-medium">{row.student.name}</p>
							<p class="text-xs text-muted-foreground">
								{row.student.homeroomName ?? 'ไม่มีห้องประจำชั้น'} · ลงทะเบียน {formatCalendarDate(
									currentAttendanceDate(new Date(row.enrolledAt))
								)}
							</p>
						</div>
						<Button
							type="button"
							variant="outline"
							class="text-destructive"
							{disabled}
							onclick={() => onremove(row.studentId)}
							aria-label={`ถอนข้อมูลใบหน้า ${row.student.name}`}>ถอนข้อมูลใบหน้า</Button
						>
					</li>{/each}
			</ul>{/if}
		{#if pages > 1}<div class="flex flex-wrap items-center justify-between gap-2">
				<p class="text-sm text-muted-foreground">
					หน้า {currentPage}/{pages} · {filtered.length} คน
				</p>
				<div class="flex gap-2">
					<Button
						type="button"
						variant="outline"
						disabled={currentPage <= 1}
						onclick={() => (page = currentPage - 1)}>ก่อนหน้า</Button
					><Button
						type="button"
						variant="outline"
						disabled={currentPage >= pages}
						onclick={() => (page = currentPage + 1)}>ถัดไป</Button
					>
				</div>
			</div>{/if}
	</CardContent>
</Card>
