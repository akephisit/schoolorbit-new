<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import CalendarTimeInput from '#lib/components/calendar/CalendarTimeInput.svelte';
	import { normalizeCalendarTime } from '#lib/utils/calendar.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Plus, Trash2 } from '@lucide/svelte';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import type { AttendanceRead } from './attendance-access.js';
	import type { AttendanceDay } from '#lib/api/attendance.js';
	import type { AttendanceCalendarEvents } from './attendance-settings-calendar.js';
	import AttendanceSettingsCalendar from './AttendanceSettingsCalendar.svelte';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { saveAttendanceSettings, type AttendanceSettings } from '#lib/api/attendance.js';
	let {
		term,
		date,
		initial,
		initialDays,
		initialCalendar
	}: {
		term: string;
		date: string;
		initial: AttendanceSettings;
		initialDays: Promise<RouteLoadResult<AttendanceRead<AttendanceDay[]>>>;
		initialCalendar: Promise<RouteLoadResult<AttendanceRead<AttendanceCalendarEvents>>>;
	} = $props();
	let settings = $state(untrack(() => initial)),
		configuration = $state(
			untrack(() => ({
				...$state.snapshot(initial.configuration),
				lateAfter: initial.configuration.lateAfter.slice(0, 5)
			}))
		),
		busy = $state(false),
		pendingAction = $state(''),
		settingsError = $state(''),
		fieldErrors = $state<Record<string, string>>({}),
		digestTimes = $state(
			untrack(() => initial.configuration.digestTimes.map((t) => t.slice(0, 5)))
		);
	const manager = $derived($can.has(PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL));
	const weekdays = ['จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส', 'อา'];
	let disposed = false;
	onDestroy(() => {
		disposed = true;
	});
	async function run(name: string, action: () => Promise<void>) {
		if (disposed || busy || settings.archived || !manager) return;
		busy = true;
		pendingAction = name;
		if (name === 'settings') settingsError = '';

		try {
			await action();
			if (disposed) return;
			toast.success('บันทึกแล้ว');
		} catch (e) {
			if (disposed) return;
			const message = e instanceof Error ? e.message : 'บันทึกไม่ได้';
			if (name === 'settings') settingsError = message;
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
			const normalizedLateAfter = normalizeCalendarTime(configuration.lateAfter);
			const normalizedDigests = digestTimes.map(normalizeCalendarTime);
			if (!normalizedLateAfter) fieldErrors.lateAfter = 'ระบุเวลาเข้าสายให้ถูกต้อง';
			if (
				digestTimes.length > 8 ||
				normalizedDigests.some((t) => !t) ||
				new Set(normalizedDigests).size !== digestTimes.length
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
				lateAfter: normalizedLateAfter + ':00',
				digestTimes: normalizedDigests.map((t) => t + ':00')
			};
			const saved = await saveAttendanceSettings(term, {
				configuration: savedConfiguration,
				rowVersion: settings.rowVersion
			});
			if (disposed) return;
			settings = saved;
			configuration = {
				...$state.snapshot(settings.configuration),
				lateAfter: settings.configuration.lateAfter.slice(0, 5)
			};
			digestTimes = settings.configuration.digestTimes.map((t) => t.slice(0, 5));
		});
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
						<div class="grid gap-6 sm:grid-cols-2">
							<div class="space-y-2">
								<Label for="attendance-late-after"
									>เข้าสายหลังเวลา <span aria-hidden="true" class="text-destructive">*</span></Label
								><CalendarTimeInput
									id="attendance-late-after"
									bind:value={configuration.lateAfter}
									disabled={busy || settings.archived}
								/>{#if fieldErrors.lateAfter}<p
										id="attendance-late-error"
										class="text-sm text-destructive"
									>
										{fieldErrors.lateAfter}
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
						<section
							aria-labelledby="attendance-digest-heading"
							class="space-y-4 rounded-xl border bg-muted/20 p-4 sm:p-5"
						>
							<div class="flex flex-wrap items-start justify-between gap-3">
								<div class="space-y-1">
									<h3 id="attendance-digest-heading" class="text-sm font-semibold">
										เวลาส่งสรุปครู
									</h3>
									<p class="text-sm text-muted-foreground">
										กำหนดได้สูงสุด 8 เวลา ใช้รูปแบบ 24 ชั่วโมง
									</p>
								</div>
								<Button
									type="button"
									variant="outline"
									disabled={digestTimes.length >= 8 || busy || settings.archived}
									onclick={() => {
										digestTimes = [...digestTimes, ''];
									}}><Plus class="size-4" />เพิ่มเวลาสรุป</Button
								>
							</div>
							{#if digestTimes.length}<div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
									{#each digestTimes as _, i (i)}<div
											class="min-w-0 space-y-3 rounded-lg border bg-card p-3"
										>
											<div class="flex items-center justify-between gap-3">
												<Label for={`attendance-digest-${i}`}>เวลาสรุป {i + 1}</Label>
												<Button
													type="button"
													variant="ghost"
													size="icon"
													aria-label={`ลบเวลาสรุป ${i + 1}`}
													disabled={busy || settings.archived}
													onclick={() => {
														digestTimes = digestTimes.filter((_, index) => index !== i);
													}}><Trash2 class="size-4" /></Button
												>
											</div>
											<div class="space-y-2">
												<CalendarTimeInput
													id={`attendance-digest-${i}`}
													bind:value={digestTimes[i]}
													disabled={busy || settings.archived}
												/>
											</div>
										</div>{/each}
								</div>{:else}<p class="text-sm text-muted-foreground">
									ยังไม่มีเวลาส่งสรุป เพิ่มเวลาเมื่อต้องการเปิดส่งสรุปครู
								</p>{/if}
							{#if fieldErrors.digests}<p
									id="attendance-digest-error"
									class="text-sm text-destructive"
								>
									{fieldErrors.digests}
								</p>{/if}
						</section>
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
		<AttendanceSettingsCalendar
			{term}
			{date}
			{initialDays}
			{initialCalendar}
			{settings}
			onbusy={(value) => {
				busy = value;
			}}
			{busy}
			onsettings={(saved) => {
				settings = saved;
			}}
		/>
	</div>
{/if}
