<script lang="ts">
	import type { GradeLevel } from '$lib/api/academic';
	import type {
		BlockedWindowInput,
		ExamDayDetail,
		UpsertExamDayInput
	} from '$lib/api/examSchedule';
	import { LoadingButton, PageState } from '$lib/components/app-state';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { compareExamDaysByDate } from '$lib/utils/examScheduleDayOrder';
	import { Plus, Trash2 } from 'lucide-svelte';

	type BlockedWindowForm = BlockedWindowInput & { localId: string };

	let {
		days = [],
		gradeLevels = [],
		readonly = false,
		saving = false,
		deletingDayId = null,
		onSaveDay,
		onDeleteDay
	}: {
		days: ExamDayDetail[];
		gradeLevels: GradeLevel[];
		readonly?: boolean;
		saving?: boolean;
		deletingDayId?: string | null;
		onSaveDay?: (examDayId: string | null, input: UpsertExamDayInput) => Promise<boolean> | boolean;
		onDeleteDay?: (examDayId: string) => Promise<void> | void;
	} = $props();

	let selectedDayId = $state('');
	let originalExamDate = $state('');
	let examDate = $state('');
	let label = $state('');
	let startTime = $state('08:30');
	let endTime = $state('16:00');
	let gradeLevelIds = $state<string[]>([]);
	let blockedWindows = $state<BlockedWindowForm[]>([]);
	let pendingDayInput = $state<UpsertExamDayInput | null>(null);
	let moveDayDialogOpen = $state(false);
	let nextWindowIndex = 0;

	const sortedDays = $derived([...days].sort(compareExamDaysByDate));
	const formTitle = $derived(selectedDayId ? 'แก้ไขวันสอบ' : 'เพิ่มวันสอบ');
	const conflictingDay = $derived(
		days.find((day) => day.id !== selectedDayId && day.examDate === examDate) ?? null
	);

	function newLocalId(): string {
		nextWindowIndex += 1;
		return `blocked-${nextWindowIndex}`;
	}

	function resetForm() {
		selectedDayId = '';
		originalExamDate = '';
		examDate = '';
		label = '';
		startTime = '08:30';
		endTime = '16:00';
		gradeLevelIds = [];
		blockedWindows = [];
	}

	function loadDay(day: ExamDayDetail) {
		selectedDayId = day.id;
		originalExamDate = day.examDate;
		examDate = day.examDate;
		label = day.label ?? '';
		startTime = day.startTime.slice(0, 5);
		endTime = day.endTime.slice(0, 5);
		gradeLevelIds = [...day.gradeLevelIds];
		blockedWindows = day.blockedWindows.map((window) => ({
			label: window.label,
			startTime: window.startTime.slice(0, 5),
			endTime: window.endTime.slice(0, 5),
			localId: newLocalId()
		}));
	}

	function toggleGradeLevel(gradeLevelId: string, checked: boolean) {
		gradeLevelIds = checked
			? Array.from(new Set([...gradeLevelIds, gradeLevelId]))
			: gradeLevelIds.filter((id) => id !== gradeLevelId);
	}

	function addBlockedWindow() {
		blockedWindows = [
			...blockedWindows,
			{ localId: newLocalId(), label: 'พัก', startTime: '12:00', endTime: '13:00' }
		];
	}

	function updateBlockedWindow(localId: string, patch: Partial<BlockedWindowInput>) {
		blockedWindows = blockedWindows.map((window) =>
			window.localId === localId ? { ...window, ...patch } : window
		);
	}

	function removeBlockedWindow(localId: string) {
		blockedWindows = blockedWindows.filter((window) => window.localId !== localId);
	}

	function dayInput(): UpsertExamDayInput {
		return {
			examDate,
			label: label.trim() || null,
			startTime,
			endTime,
			gradeLevelIds,
			blockedWindows: blockedWindows
				.filter((window) => window.label.trim() && window.startTime && window.endTime)
				.map((window) => ({
					label: window.label.trim(),
					startTime: window.startTime,
					endTime: window.endTime
				}))
		};
	}

	async function saveDay(input: UpsertExamDayInput) {
		const saved = await onSaveDay?.(selectedDayId || null, input);
		if (saved) resetForm();
	}

	async function submitForm() {
		if (!examDate || !startTime || !endTime || conflictingDay) return;

		const input = dayInput();
		if (selectedDayId && examDate !== originalExamDate) {
			pendingDayInput = input;
			moveDayDialogOpen = true;
			return;
		}

		await saveDay(input);
	}

	async function confirmMoveDay() {
		const input = pendingDayInput;
		pendingDayInput = null;
		moveDayDialogOpen = false;
		if (input) await saveDay(input);
	}

	function cancelMoveDay() {
		pendingDayInput = null;
	}

	function formatDayDate(value: string): string {
		return new Date(value).toLocaleDateString('th-TH', {
			weekday: 'short',
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	function gradeNames(ids: string[]): string {
		if (ids.length === 0) return 'ทุกระดับ';
		return ids
			.map((id) => gradeLevels.find((gradeLevel) => gradeLevel.id === id)?.short_name ?? '')
			.filter(Boolean)
			.join(', ');
	}

	$effect(() => {
		if (selectedDayId && !days.some((day) => day.id === selectedDayId)) {
			resetForm();
		}
	});
</script>

<section class="overflow-hidden rounded-md border bg-background">
	<div
		class="flex flex-col gap-3 border-b px-4 py-4 md:flex-row md:items-center md:justify-between"
	>
		<div>
			<h2 class="font-semibold">วันสอบ</h2>
			<p class="text-sm text-muted-foreground">{days.length} วัน</p>
		</div>
		{#if !readonly}
			<Button variant="outline" size="sm" onclick={resetForm}>
				<Plus class="h-4 w-4" />
				วันใหม่
			</Button>
		{/if}
	</div>

	<div class="grid gap-0 lg:grid-cols-[minmax(0,1fr)_22rem]">
		<div class="min-w-0 border-b lg:border-b-0 lg:border-r">
			{#if sortedDays.length === 0}
				<PageState title="ยังไม่มีวันสอบ" description="เพิ่มวันสอบเพื่อกำหนดช่วงเวลาและระดับชั้น" />
			{:else}
				<div class="overflow-x-auto">
					<Table class="min-w-[680px]">
						<TableHeader>
							<TableRow>
								<TableHead>วันสอบ</TableHead>
								<TableHead class="w-36">เวลา</TableHead>
								<TableHead>ระดับชั้น</TableHead>
								<TableHead class="w-28 text-center">ช่วงปิด</TableHead>
								{#if !readonly}
									<TableHead class="w-32 text-right">จัดการ</TableHead>
								{/if}
							</TableRow>
						</TableHeader>
						<TableBody>
							{#each sortedDays as day (day.id)}
								<TableRow>
									<TableCell>
										<div class="font-medium">{day.label || formatDayDate(day.examDate)}</div>
										{#if day.label}
											<div class="text-xs text-muted-foreground">{formatDayDate(day.examDate)}</div>
										{/if}
									</TableCell>
									<TableCell class="font-mono text-sm">
										{day.startTime.slice(0, 5)}-{day.endTime.slice(0, 5)}
									</TableCell>
									<TableCell class="text-sm">{gradeNames(day.gradeLevelIds)}</TableCell>
									<TableCell class="text-center">
										<Badge variant="outline">{day.blockedWindows.length}</Badge>
									</TableCell>
									{#if !readonly}
										<TableCell class="text-right">
											<div class="flex justify-end gap-1">
												<Button variant="outline" size="sm" onclick={() => loadDay(day)}
													>แก้ไข</Button
												>
												<LoadingButton
													variant="ghost"
													size="icon-sm"
													loading={deletingDayId === day.id}
													loadingLabel=""
													onclick={() => onDeleteDay?.(day.id)}
													aria-label="ลบวันสอบ"
												>
													<Trash2 class="h-4 w-4 text-destructive" />
												</LoadingButton>
											</div>
										</TableCell>
									{/if}
								</TableRow>
							{/each}
						</TableBody>
					</Table>
				</div>
			{/if}
		</div>

		<div class="p-4">
			{#if readonly}
				<PageState
					variant="permission"
					title="อ่านอย่างเดียว"
					description="ผู้ใช้ปัจจุบันไม่มีสิทธิ์แก้ไขการตั้งค่าวันสอบ"
				/>
			{:else}
				<form
					class="space-y-4"
					onsubmit={(event) => {
						event.preventDefault();
						submitForm();
					}}
				>
					<div>
						<h3 class="text-sm font-semibold">{formTitle}</h3>
					</div>
					<div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
						<div class="grid gap-2">
							<Label for="exam-day-date">วันที่</Label>
							<DatePicker id="exam-day-date" bind:value={examDate} placeholder="เลือกวันสอบ" />
							{#if conflictingDay}
								<p class="text-xs text-destructive">
									วันที่นี้มีวันสอบอยู่แล้ว กรุณาย้ายวันนั้นไปวันที่ว่างก่อน
								</p>
							{/if}
						</div>
						<div class="grid gap-2">
							<Label for="exam-day-label">ป้ายชื่อ</Label>
							<Input id="exam-day-label" bind:value={label} placeholder="เว้นว่างได้" />
						</div>
						<div class="grid grid-cols-2 gap-3">
							<div class="grid gap-2">
								<Label for="exam-day-start">เริ่ม</Label>
								<Input id="exam-day-start" type="time" bind:value={startTime} required />
							</div>
							<div class="grid gap-2">
								<Label for="exam-day-end">สิ้นสุด</Label>
								<Input id="exam-day-end" type="time" bind:value={endTime} required />
							</div>
						</div>
					</div>

					<div class="space-y-2">
						<Label>ระดับชั้น</Label>
						<div class="grid max-h-44 gap-2 overflow-y-auto rounded-md border p-3">
							{#each gradeLevels as gradeLevel (gradeLevel.id)}
								<label class="flex items-center gap-2 text-sm">
									<Checkbox
										checked={gradeLevelIds.includes(gradeLevel.id)}
										onCheckedChange={(checked) => toggleGradeLevel(gradeLevel.id, checked === true)}
									/>
									<span>{gradeLevel.short_name} · {gradeLevel.name}</span>
								</label>
							{/each}
						</div>
					</div>

					<div class="space-y-2">
						<div class="flex items-center justify-between gap-2">
							<Label>ช่วงเวลาปิด</Label>
							<Button type="button" variant="outline" size="sm" onclick={addBlockedWindow}>
								<Plus class="h-4 w-4" />
								เพิ่ม
							</Button>
						</div>
						<div class="space-y-2">
							{#each blockedWindows as window (window.localId)}
								<div class="grid gap-2 rounded-md border p-2">
									<Input
										value={window.label}
										placeholder="ชื่อช่วง"
										oninput={(event) =>
											updateBlockedWindow(window.localId, {
												label: (event.currentTarget as HTMLInputElement).value
											})}
									/>
									<div class="grid grid-cols-[1fr_1fr_auto] gap-2">
										<Input
											type="time"
											value={window.startTime}
											oninput={(event) =>
												updateBlockedWindow(window.localId, {
													startTime: (event.currentTarget as HTMLInputElement).value
												})}
										/>
										<Input
											type="time"
											value={window.endTime}
											oninput={(event) =>
												updateBlockedWindow(window.localId, {
													endTime: (event.currentTarget as HTMLInputElement).value
												})}
										/>
										<Button
											type="button"
											variant="ghost"
											size="icon"
											onclick={() => removeBlockedWindow(window.localId)}
											aria-label="ลบช่วงเวลาปิด"
										>
											<Trash2 class="h-4 w-4" />
										</Button>
									</div>
								</div>
							{/each}
						</div>
					</div>

					<LoadingButton
						type="submit"
						loading={saving}
						loadingLabel="กำลังบันทึก..."
						disabled={!examDate || !startTime || !endTime || Boolean(conflictingDay)}
						class="w-full"
					>
						บันทึกวันสอบ
					</LoadingButton>
				</form>
			{/if}
		</div>
	</div>
</section>

<AlertDialog.Root bind:open={moveDayDialogOpen}>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ยืนยันการย้ายวันสอบ</AlertDialog.Title>
			<AlertDialog.Description>
				ย้ายจาก {formatDayDate(originalExamDate)} เป็น {formatDayDate(examDate)}? วิชา ห้องสอบ
				กรรมการคุมสอบ และเลขที่นั่งของวันนี้จะย้ายตามไปทั้งหมด
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel onclick={cancelMoveDay}>ยกเลิก</AlertDialog.Cancel>
			<AlertDialog.Action onclick={confirmMoveDay}>ย้ายวันสอบ</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
