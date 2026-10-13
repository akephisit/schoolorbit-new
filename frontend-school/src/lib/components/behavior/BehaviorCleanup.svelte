<script lang="ts">
	import { ApiClientError } from '#lib/api/client.js';
	import { onDestroy } from 'svelte';
	import {
		previewBehaviorCleanup,
		executeBehaviorCleanup,
		previewBehaviorReset,
		resetBehaviorScores,
		behaviorCleanupStatus,
		type BehaviorCleanupPreview,
		type BehaviorCleanupOutcome,
		type BehaviorCleanupScope,
		type BehaviorCleanupMode
	} from '#lib/api/behavior.js';
	import {
		sides,
		sources,
		sideLabels,
		sourceLabels,
		modes,
		modeLabels,
		bangkokDateTime
	} from '#lib/features/behavior/presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import BehaviorSelect from './BehaviorSelect.svelte';
	let { term, year }: { term: string | null; year: string | null } = $props();
	let reset = $state(false);
	let start = $state(bangkokDateTime().date.slice(0, 7) + '-01');
	let allDates = $state(false);
	let end = $state(bangkokDateTime().date);
	let termOnly = $state(true);
	let chosenSides = $state([...sides]);
	let chosenSources = $state([...sources]);
	let mode = $state<BehaviorCleanupMode>('archive');
	let reason = $state('');
	let preview = $state<BehaviorCleanupPreview | null>(null);
	let previewKey = $state('');
	let previewScope = $state.raw<BehaviorCleanupScope | null>(null);
	let previewMode = $state<BehaviorCleanupMode>('archive');
	let previewReset = $state(false);
	let pending = $state('');
	let error = $state('');
	let confirm = $state(false);
	let outcome = $state<BehaviorCleanupOutcome | null>(null);
	let requestId = $state('');
	let disposed = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	const controller = new AbortController();
	onDestroy(() => {
		disposed = true;
		controller.abort();
		clearTimeout(timer);
	});
	const scope = $derived<BehaviorCleanupScope>({
		start: !reset && allDates ? null : start || null,
		end: !reset && allDates ? null : end || null,
		academicYearId: termOnly ? year : null,
		academicTermId: termOnly ? term : null,
		sides: chosenSides,
		sources: chosenSources
	});
	const key = $derived(JSON.stringify({ reset, scope, mode }));
	async function inspect(e: SubmitEvent) {
		e.preventDefault();
		if (pending) return;
		pending = 'preview';
		error = '';
		preview = null;
		try {
			const snap = { ...scope, sides: [...chosenSides], sources: [...chosenSources] };
			const result = reset
				? await previewBehaviorReset({ sides: chosenSides, through: end })
				: await previewBehaviorCleanup({ scope: snap, mode });
			if (disposed) return;
			preview = result;
			previewScope = snap;
			previewMode = mode;
			previewReset = reset;
			previewKey = key;
			requestId = '';
			outcome = null;
		} catch (e) {
			if (!disposed) error = e instanceof Error ? e.message : 'ตรวจผลกระทบไม่ได้';
		} finally {
			if (!disposed) pending = '';
		}
	}
	async function poll(id: string) {
		try {
			const result = await behaviorCleanupStatus(id, { signal: controller.signal });
			if (disposed) return;
			outcome = result;
			if (result.status === 'pending') timer = setTimeout(() => void poll(id), 1500);
			else {
				pending = '';
				confirm = false;
				preview = null;
			}
		} catch (e) {
			if (!disposed) {
				pending = '';
				error = e instanceof Error ? e.message : 'โหลดสถานะไม่ได้';
			}
		}
	}
	async function execute() {
		if (!preview || !previewScope || previewKey !== key || !reason.trim() || pending) return;
		pending = 'execute';
		error = '';
		try {
			requestId ||= crypto.randomUUID();
			const result = previewReset
				? await resetBehaviorScores({
						requestId,
						token: preview.token,
						revision: preview.revision,
						sides: previewScope.sides,
						through: previewScope.end ?? end,
						reason
					})
				: await executeBehaviorCleanup({
						requestId,
						token: preview.token,
						revision: preview.revision,
						scope: previewScope,
						mode: previewMode,
						reason
					});
			if (disposed) return;
			outcome = result;
			if (result.status === 'pending') {
				pending = 'job';
				void poll(result.id);
			} else {
				pending = '';
				confirm = false;
				preview = null;
			}
		} catch (e) {
			if (!disposed) {
				error = e instanceof Error ? e.message : 'ดำเนินการไม่ได้';
				if (e instanceof ApiClientError && e.status < 500) {
					requestId = '';
					if (e.status === 409) preview = null;
				}
				pending = '';
			}
		}
	}
</script>

<form class="mx-auto max-w-4xl space-y-4" onsubmit={inspect}>
	<h2 class="text-lg font-semibold">ขอบเขตข้อมูล</h2>
	<fieldset disabled={!!pending || !!requestId} class="space-y-4">
		<label class="flex min-h-11 items-center gap-2"
			><Checkbox bind:checked={reset} />รีเซ็ตคะแนน เปิดรอบใหม่เริ่มศูนย์ และเก็บยอดเดิมเป็นประวัติ</label
		>{#if reset}<p class="text-sm text-muted-foreground">
				รีเซ็ตฝั่งที่เลือกทั้งโรงเรียน ต้องปิดยอดเช็กชื่อถึงวันที่เลือกก่อน
			</p>{:else}<BehaviorSelect
				id="cleanup-mode"
				label="วิธีล้างข้อมูล"
				value={mode}
				options={modes.map((m) => ({ value: m, label: modeLabels[m] }))}
				onValueChange={(v) => {
					const m = modes.find((m) => m === v);
					if (m) mode = m;
				}}
			/><label class="flex min-h-11 items-center gap-2"
				><Checkbox bind:checked={termOnly} />เฉพาะภาคเรียนที่เลือก (ปิดเพื่อเลือกทุกภาคเรียน)</label
			>{/if}
		{#if !reset}<label class="flex min-h-11 items-center gap-2"
				><Checkbox bind:checked={allDates} />ทุกวันในขอบเขตที่เลือก</label
			>{/if}
		<div class="grid gap-3 sm:grid-cols-2">
			{#if !reset}<div>
					<Label for="cleanup-start">เริ่มวันที่</Label><DatePicker
						id="cleanup-start"
						value={start}
						onValueChange={(v) => {
							if (v) start = v;
						}}
					/>
				</div>{/if}
			<div>
				<Label for="cleanup-end">ถึงวันที่ *</Label><DatePicker
					id="cleanup-end"
					value={end}
					onValueChange={(v) => {
						if (v) end = v;
					}}
					required
				/>
			</div>
		</div>
		<div class="flex flex-wrap gap-4">
			{#each sides as side (side)}<label class="flex min-h-11 items-center gap-2"
					><Checkbox
						checked={chosenSides.includes(side)}
						onCheckedChange={(v) =>
							(chosenSides = v ? [...chosenSides, side] : chosenSides.filter((s) => s !== side))}
					/>{sideLabels[side]}</label
				>{/each}
		</div>
		{#if !reset}<div class="flex flex-wrap gap-4">
				{#each sources as source (source)}<label class="flex min-h-11 items-center gap-2"
						><Checkbox
							checked={chosenSources.includes(source)}
							onCheckedChange={(v) =>
								(chosenSources = v
									? [...chosenSources, source]
									: chosenSources.filter((s) => s !== source))}
						/>{sourceLabels[source]}</label
					>{/each}
			</div>{/if}
	</fieldset>
	<LoadingButton
		type="submit"
		variant="outline"
		loading={pending === 'preview'}
		disabled={!!pending || !chosenSides.length || (!reset && !chosenSources.length)}
		>ตรวจผลกระทบก่อนดำเนินการ</LoadingButton
	>
</form>
{#if error}<PageState variant="error" title="ดำเนินการไม่สำเร็จ" description={error} />{/if}
{#if preview}<section class="mx-auto max-w-4xl space-y-3 rounded-md border p-4">
		<h2 class="font-semibold">ผลกระทบที่ตรวจแล้ว</h2>
		<p>
			{preview.students} คน · {preview.entries} รายการ · ความดี {preview.merit} · ถูกหัก {preview.deduction}
		</p>
		<p class="text-sm">
			ยอดในขอบเขต: ปัจจุบัน {preview.current.merit}/{preview.current.deduction} · ประวัติ {preview
				.history.merit}/{preview.history.deduction} · สะสม {preview.lifetime.merit}/{preview
				.lifetime.deduction}
		</p>
		<p class="text-sm text-muted-foreground">
			รายละเอียดที่ลบแล้วจะเปิดดูหรือยกเลิกรายเหตุการณ์ไม่ได้ ข้อมูลเช็กชื่อยังอยู่ในระบบเช็กชื่อ
		</p>
		<Label for="cleanup-reason">เหตุผล *</Label><Textarea
			id="cleanup-reason"
			bind:value={reason}
			required
			maxlength={1000}
			disabled={!!pending || !!requestId}
		/>{#if previewKey !== key}<p class="text-warning">
				ขอบเขตเปลี่ยน กรุณาตรวจผลกระทบใหม่
			</p>{/if}<Button
			type="button"
			variant="destructive"
			disabled={!!pending || previewKey !== key || !reason.trim()}
			onclick={() => (confirm = true)}>ดำเนินการตามผลที่ตรวจ</Button
		>
	</section>{/if}
{#if outcome}<section role="status" class="rounded-md border p-4">
		<h2 class="font-semibold">
			{outcome.status === 'pending' ? 'กำลังจัดการข้อมูล' : 'จัดการข้อมูลสำเร็จ'}
		</h2>
		<p>ลบรายละเอียดแล้ว {outcome.entriesDeleted} รายการ · นักเรียน {outcome.students} คน</p>
		{#if outcome.status === 'pending' && !pending}<Button
				type="button"
				variant="outline"
				onclick={() => {
					pending = 'job';
					void poll(outcome!.id);
				}}>ตรวจสถานะต่อ</Button
			>{/if}
	</section>{/if}
<AlertDialog.Root bind:open={confirm}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ยืนยัน{previewReset ? 'รีเซ็ตคะแนน' : 'ล้างข้อมูล'}</AlertDialog.Title
			><AlertDialog.Description
				>กระทบ {preview?.students ?? 0} คน และลบรายละเอียดจริงตามขอบเขตที่ตรวจแล้ว</AlertDialog.Description
			></AlertDialog.Header
		>{#if pending === 'job'}<p role="status">
				กำลังดำเนินการ ลบแล้ว {outcome?.entriesDeleted ?? 0} รายการ
			</p>{/if}{#if error}<p class="text-destructive">{error}</p>{/if}<AlertDialog.Footer
			><AlertDialog.Cancel disabled={!!pending}>กลับ</AlertDialog.Cancel><LoadingButton
				type="button"
				variant="destructive"
				loading={!!pending}
				onclick={() => void execute()}>ยืนยันดำเนินการ</LoadingButton
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
