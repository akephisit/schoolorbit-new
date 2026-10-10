<script lang="ts">
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { onDestroy, untrack } from 'svelte';
	import { SvelteDate } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		attendanceDays,
		saveAttendanceDays,
		saveAttendanceSettings,
		type AttendanceSettings,
		type AttendanceDay
	} from '#lib/api/attendance.js';
	let {
		term,
		date,
		initial
	}: {
		term: string;
		date: string;
		initial: [AttendanceSettings, AttendanceDay[]];
	} = $props();
	let settings = $state(untrack(() => initial[0])),
		days = $state(untrack(() => initial[1])),
		configuration = $state(untrack(() => $state.snapshot(initial[0].configuration))),
		month = $state(untrack(() => date.slice(0, 7))),
		note = $state(''),
		busy = $state(false),
		pendingAction = $state(''),
		calendarError = $state(''),
		settingsError = $state(''),
		daysError = $state(''),
		fieldErrors = $state<Record<string, string>>({}),
		calendarLoading = $state(false),
		digests = $state(
			untrack(() => initial[0].configuration.digestTimes.map((t) => t.slice(0, 5)).join(', '))
		);
	let rangeStart = $state(untrack(() => date)),
		rangeEnd = $state(untrack(() => date));
	const manager = $derived($can.has(PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL));
	const cells = $derived.by(() => {
		const [year, m] = month.split('-').map(Number);
		if (!/^\d{4}-\d{2}$/.test(month) || m < 1 || m > 12) return [];
		const start = new Date(Date.UTC(year, m - 1, 1));
		const total = new Date(Date.UTC(year, m, 0)).getUTCDate();
		return [
			...Array((start.getUTCDay() + 6) % 7).fill(null),
			...Array.from({ length: total }, (_, i) => `${month}-${String(i + 1).padStart(2, '0')}`)
		] as (string | null)[];
	});
	let disposed = false;
	const calendarRequest = new LatestRequest();
	onDestroy(() => {
		disposed = true;
		calendarRequest.abort();
	});
	const weekdays = ['จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส', 'อา'];
	function isCounted(value: string) {
		return (
			days.find((d) => d.date === value)?.counted ??
			settings.configuration.weekdays.includes(
				((new Date(value + 'T00:00:00Z').getUTCDay() + 6) % 7) + 1
			)
		);
	}
	async function run(name: string, action: () => Promise<void>) {
		if (disposed || busy || settings.archived || !manager) return;
		busy = true;
		pendingAction = name;
		if (name === 'settings') settingsError = '';
		else daysError = '';
		try {
			await action();
			if (disposed) return;
			toast.success('บันทึกแล้ว');
		} catch (e) {
			if (disposed) return;
			const message = e instanceof Error ? e.message : 'บันทึกไม่ได้';
			if (name === 'settings') settingsError = message;
			else daysError = message;
		} finally {
			if (!disposed) {
				busy = false;
				pendingAction = '';
			}
		}
	}
	async function saveConfig() {
		await run('settings', async () => {
			fieldErrors = {};
			const timePattern = /^([01]\d|2[0-3]):[0-5]\d(?::[0-5]\d)?$/;
			const digestTimes = digests
				.split(',')
				.map((t) => t.trim())
				.filter(Boolean);
			if (!timePattern.test(configuration.lateAfter))
				fieldErrors.lateAfter = 'ระบุเวลาเข้าสายให้ถูกต้อง';
			if (
				digestTimes.length > 8 ||
				digestTimes.some((t) => !timePattern.test(t)) ||
				new Set(digestTimes.map((t) => (t.length === 5 ? t + ':00' : t))).size !==
					digestTimes.length
			)
				fieldErrors.digests = 'ระบุเวลาไม่ซ้ำกัน ไม่เกิน 8 เวลา เช่น 09:00, 16:00';
			if (
				!Number.isInteger(configuration.evidenceDays) ||
				configuration.evidenceDays < 1 ||
				configuration.evidenceDays > 365
			)
				fieldErrors.evidenceDays = 'ระบุจำนวนวันตั้งแต่ 1 ถึง 365';
			if (
				!Number.isFinite(configuration.faceDistance) ||
				configuration.faceDistance < 0.2 ||
				configuration.faceDistance > 0.6
			)
				fieldErrors.faceDistance = 'ระบุค่าตั้งแต่ 0.2 ถึง 0.6';
			if (
				!Number.isFinite(configuration.faceMargin) ||
				configuration.faceMargin < 0.05 ||
				configuration.faceMargin > 0.3
			)
				fieldErrors.faceMargin = 'ระบุค่าตั้งแต่ 0.05 ถึง 0.3';
			if (Object.keys(fieldErrors).length) throw new Error('ตรวจข้อมูลการตั้งค่าที่ระบุไว้');
			const savedConfiguration = {
				...$state.snapshot(configuration),
				lateAfter:
					configuration.lateAfter.length === 5
						? configuration.lateAfter + ':00'
						: configuration.lateAfter,
				digestTimes: digestTimes.map((t) => (t.length === 5 ? t + ':00' : t))
			};
			const saved = await saveAttendanceSettings(term, {
				configuration: savedConfiguration,
				rowVersion: settings.rowVersion
			});
			if (disposed) return;
			settings = saved;
			configuration = $state.snapshot(settings.configuration);
		});
	}
	async function loadMonth() {
		if (disposed) return;
		const t = calendarRequest.begin();
		calendarError = '';
		daysError = '';
		calendarLoading = true;
		try {
			if (!/^\d{4}-\d{2}$/.test(month)) throw new Error('เลือกเดือนที่ต้องการ');
			const [year, m] = month.split('-').map(Number);
			const end = new Date(Date.UTC(year, m, 0)).toISOString().slice(0, 10);
			const result = await attendanceDays(term, month + '-01', end, { signal: t.signal });
			if (calendarRequest.isCurrent(t.revision)) days = result;
		} catch (e) {
			if (calendarRequest.isCurrent(t.revision))
				calendarError = e instanceof Error ? e.message : 'โหลดปฏิทินไม่ได้';
		} finally {
			if (calendarRequest.isCurrent(t.revision)) calendarLoading = false;
		}
	}
	async function setDays(values: AttendanceDay[]) {
		if (disposed || calendarLoading || calendarError) return;
		if (new TextEncoder().encode(note).length > 500) {
			daysError = 'หมายเหตุยาวเกินกำหนด กรุณาย่อข้อความ';
			return;
		}
		await run('days', async () => {
			const saved = await saveAttendanceDays(term, {
				days: values,
				rowVersion: settings.rowVersion
			});
			if (disposed) return;
			settings = saved;
			days = [...days.filter((d) => !values.some((v) => v.date === d.date)), ...values];
		});
	}
	function dateRange(start: string, end: string) {
		if (
			!start ||
			!end ||
			!Number.isFinite(Date.parse(start + 'T00:00:00Z')) ||
			!Number.isFinite(Date.parse(end + 'T00:00:00Z')) ||
			start > end
		)
			throw new Error('เลือกช่วงวันที่ให้ถูกต้อง');
		const values: string[] = [];
		const current = new SvelteDate(start + 'T00:00:00Z');
		const stop = new Date(end + 'T00:00:00Z');
		while (current <= stop && values.length <= 366) {
			values.push(current.toISOString().slice(0, 10));
			current.setUTCDate(current.getUTCDate() + 1);
		}
		if (values.length > 366) throw new Error('เลือกครั้งละไม่เกิน 366 วัน');
		return values;
	}
	async function applyRange(value: boolean) {
		if (disposed) return;
		try {
			await setDays(
				dateRange(rangeStart, rangeEnd).map((d) => ({ date: d, counted: value, note }))
			);
		} catch (e) {
			if (disposed) return;
			daysError = e instanceof Error ? e.message : 'ช่วงวันที่ไม่ถูกต้อง';
		}
	}
</script>

{#if !manager}
	<PageState variant="permission" title="ต้องมีสิทธิ์ตั้งค่าระบบเช็คชื่อทั้งโรงเรียน" />
{:else}
	<div class="min-w-0 space-y-6">
		<Card.Root>
			<Card.Header>
				<Card.Title><h2>การประมวลผลและแจ้งเตือน</h2></Card.Title>
				<Card.Description
					>กำหนดวันนับยอด เวลาเข้าสาย และการแจ้งสรุปสำหรับภาคเรียนนี้</Card.Description
				>
			</Card.Header>
			<Card.Content>
				<form
					onsubmit={(event) => {
						event.preventDefault();
						void saveConfig();
					}}
					novalidate
				>
					<fieldset disabled={busy || settings.archived} class="space-y-6">
						{#if settingsError}<Alert.Root variant="destructive"
								><Alert.Title>บันทึกการตั้งค่าไม่ได้</Alert.Title><Alert.Description
									>{settingsError}</Alert.Description
								></Alert.Root
							>{/if}
						<div class="space-y-3">
							<div class="flex items-center gap-2">
								<Checkbox
									id="attendance-digest-enabled"
									bind:checked={configuration.enabled}
									disabled={settings.archived}
								/><Label for="attendance-digest-enabled">เปิดส่งสรุปครูตามเวลา</Label>
							</div>
							<p class="text-sm text-muted-foreground">
								เลือกวันเพื่อนับในยอดสรุปและส่งสรุปครู
								วันที่ไม่ได้เลือกยังบันทึกและแจ้งเตือนรายคนได้
							</p>
							<div class="flex flex-wrap gap-3">
								{#each weekdays as day, i (i)}<div class="flex items-center gap-2">
										<Checkbox
											id={`attendance-weekday-${i}`}
											checked={configuration.weekdays.includes(i + 1)}
											onCheckedChange={(checked) => {
												configuration.weekdays = checked
													? [...configuration.weekdays, i + 1]
													: configuration.weekdays.filter((day) => day !== i + 1);
											}}
										/><Label for={`attendance-weekday-${i}`}>{day}</Label>
									</div>{/each}
							</div>
						</div>
						<div class="grid gap-4 md:grid-cols-3">
							<div class="space-y-2">
								<Label for="attendance-late-after"
									>เข้าสายหลังเวลา <span aria-hidden="true" class="text-destructive">*</span></Label
								><Input
									id="attendance-late-after"
									aria-label="เข้าสายหลังเวลา"
									type="time"
									required
									bind:value={configuration.lateAfter}
									aria-invalid={!!fieldErrors.lateAfter}
									aria-describedby={fieldErrors.lateAfter ? 'attendance-late-error' : undefined}
								/>{#if fieldErrors.lateAfter}<p
										id="attendance-late-error"
										class="text-sm text-destructive"
									>
										{fieldErrors.lateAfter}
									</p>{/if}
							</div>
							<div class="space-y-2">
								<Label for="attendance-digest-times">ส่งสรุปครู (เช่น 09:00, 16:00)</Label><Input
									id="attendance-digest-times"
									bind:value={digests}
									aria-invalid={!!fieldErrors.digests}
									aria-describedby={fieldErrors.digests ? 'attendance-digest-error' : undefined}
								/>{#if fieldErrors.digests}<p
										id="attendance-digest-error"
										class="text-sm text-destructive"
									>
										{fieldErrors.digests}
									</p>{/if}
							</div>
							<div class="space-y-2">
								<Label for="attendance-evidence-days"
									>เก็บภาพหลักฐาน (วัน) <span aria-hidden="true" class="text-destructive">*</span
									></Label
								><Input
									id="attendance-evidence-days"
									aria-label="เก็บภาพหลักฐาน (วัน)"
									type="number"
									min={1}
									max={365}
									required
									bind:value={configuration.evidenceDays}
									aria-invalid={!!fieldErrors.evidenceDays}
									aria-describedby={fieldErrors.evidenceDays
										? 'attendance-evidence-error'
										: undefined}
								/>{#if fieldErrors.evidenceDays}<p
										id="attendance-evidence-error"
										class="text-sm text-destructive"
									>
										{fieldErrors.evidenceDays}
									</p>{/if}
							</div>
						</div>
						<div class="flex items-center gap-2">
							<Checkbox
								id="attendance-activity-present"
								bind:checked={configuration.activityCountsAsPresent}
							/><Label for="attendance-activity-present">นับกิจกรรมร่วมกับมาในการคำนวณร้อยละ</Label>
						</div>
						<details
							class="rounded-lg border p-4"
							open={!!fieldErrors.faceDistance || !!fieldErrors.faceMargin}
						>
							<summary class="cursor-pointer font-medium">ความเข้มงวดการจับคู่ใบหน้า</summary>
							<div class="mt-4 grid gap-4 md:grid-cols-2">
								<div class="space-y-2">
									<Label for="attendance-face-distance"
										>ระยะสูงสุด (ค่าน้อยเข้มงวดขึ้น) <span
											aria-hidden="true"
											class="text-destructive">*</span
										></Label
									><Input
										id="attendance-face-distance"
										aria-label="ระยะสูงสุด (ค่าน้อยเข้มงวดขึ้น)"
										type="number"
										step={0.01}
										min={0.2}
										max={0.6}
										required
										bind:value={configuration.faceDistance}
										aria-invalid={!!fieldErrors.faceDistance}
										aria-describedby={fieldErrors.faceDistance
											? 'attendance-distance-error'
											: undefined}
									/>{#if fieldErrors.faceDistance}<p
											id="attendance-distance-error"
											class="text-sm text-destructive"
										>
											{fieldErrors.faceDistance}
										</p>{/if}
								</div>
								<div class="space-y-2">
									<Label for="attendance-face-margin"
										>ส่วนต่างจากคนที่คล้ายกัน <span aria-hidden="true" class="text-destructive"
											>*</span
										></Label
									><Input
										id="attendance-face-margin"
										aria-label="ส่วนต่างจากคนที่คล้ายกัน"
										type="number"
										step={0.01}
										min={0.05}
										max={0.3}
										required
										bind:value={configuration.faceMargin}
										aria-invalid={!!fieldErrors.faceMargin}
										aria-describedby={fieldErrors.faceMargin
											? 'attendance-margin-error'
											: undefined}
									/>{#if fieldErrors.faceMargin}<p
											id="attendance-margin-error"
											class="text-sm text-destructive"
										>
											{fieldErrors.faceMargin}
										</p>{/if}
								</div>
							</div>
						</details>
						<div class="flex justify-end border-t pt-4">
							<LoadingButton
								type="submit"
								loading={pendingAction === 'settings'}
								disabled={busy || settings.archived}>บันทึกตั้งค่า</LoadingButton
							>
						</div>
					</fieldset>
				</form>
			</Card.Content>
		</Card.Root>
		<Card.Root>
			<Card.Header
				><Card.Title><h2>ปฏิทินวันประมวลผล</h2></Card.Title><Card.Description
					>เลือกวันที่เพื่อสลับการนับยอด หรือกำหนดพร้อมกันเป็นช่วงวันที่</Card.Description
				></Card.Header
			>
			<Card.Content class="space-y-6">
				<div class="grid gap-4 sm:grid-cols-2">
					<div class="space-y-2">
						<Label for="attendance-month">เดือน</Label><Input
							id="attendance-month"
							type="month"
							bind:value={month}
							onchange={loadMonth}
							disabled={busy}
						/>
					</div>
					<div class="space-y-2">
						<Label for="attendance-calendar-note">หมายเหตุวันหยุด / กิจกรรม</Label><Input
							id="attendance-calendar-note"
							bind:value={note}
							disabled={busy || settings.archived}
						/>
					</div>
				</div>
				{#if calendarError}<PageState
						variant="error"
						title="โหลดปฏิทินไม่ได้"
						description={calendarError}
						actionLabel="ลองโหลดปฏิทินใหม่"
						onaction={loadMonth}
					/>{/if}
				{#if daysError}<Alert.Root variant="destructive"
						><Alert.Title>บันทึกวันประมวลผลไม่ได้</Alert.Title><Alert.Description
							>{daysError}</Alert.Description
						></Alert.Root
					>{/if}
				{#if calendarLoading}<p role="status" class="text-sm text-muted-foreground">
						กำลังโหลดปฏิทิน…
					</p>{/if}
				<div aria-busy={calendarLoading} class="grid max-w-2xl grid-cols-7 gap-1 sm:gap-2">
					{#each weekdays as day (day)}<span
							class="py-2 text-center text-sm font-medium text-muted-foreground">{day}</span
						>{/each}
					{#each cells as value, index (index)}{#if value}<Button
								type="button"
								class="min-w-0 px-1 sm:px-3"
								aria-pressed={isCounted(value)}
								variant={isCounted(value) ? 'default' : 'outline'}
								disabled={busy || calendarLoading || !!calendarError || settings.archived}
								onclick={() => setDays([{ date: value, counted: !isCounted(value), note }])}
								title={days.find((d) => d.date === value)?.note ?? ''}
								>{Number(value.slice(-2))} {isCounted(value) ? '✓' : '—'}</Button
							>{:else}<span></span>{/if}{/each}
				</div>
				<div class="flex flex-wrap items-end gap-3 border-t pt-4">
					<div class="min-w-0 space-y-2">
						<Label for="attendance-range-start">จาก</Label><DatePicker
							id="attendance-range-start"
							ariaLabel="จาก"
							bind:value={rangeStart}
							disabled={busy || settings.archived}
						/>
					</div>
					<div class="min-w-0 space-y-2">
						<Label for="attendance-range-end">ถึง</Label><DatePicker
							id="attendance-range-end"
							ariaLabel="ถึง"
							bind:value={rangeEnd}
							disabled={busy || settings.archived}
						/>
					</div>
					<Button
						type="button"
						variant="outline"
						disabled={busy || calendarLoading || !!calendarError || settings.archived}
						onclick={() => applyRange(true)}>นับช่วงนี้</Button
					><Button
						type="button"
						variant="outline"
						disabled={busy || calendarLoading || !!calendarError || settings.archived}
						onclick={() => applyRange(false)}>วันหยุด / ไม่นับช่วงนี้</Button
					>
				</div>
			</Card.Content>
		</Card.Root>
	</div>
{/if}
