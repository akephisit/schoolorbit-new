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
	import BehaviorFormSection from './BehaviorFormSection.svelte';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { RotateCcw, Trash2, SearchCheck, TriangleAlert } from '@lucide/svelte';
	let { term, year }: { term: string | null; year: string | null } = $props();
	let reset = $state(false);
	function chooseOperation(value: string) {
		if (value === 'reset' || value === 'cleanup') reset = value === 'reset';
	}
	function chooseMode(value: string) {
		const selected = modes.find((candidate) => candidate === value);
		if (selected) mode = selected;
	}
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
				requestId = '';
				previewScope = null;
				previewKey = '';
				reason = '';
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
				requestId = '';
				previewScope = null;
				previewKey = '';
				reason = '';
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

<form class="max-w-5xl space-y-6" onsubmit={inspect}>
	<fieldset disabled={!!pending || !!requestId} class="min-w-0 space-y-6">
		<legend class="sr-only">วิธีจัดการและขอบเขตข้อมูลคะแนน</legend>
		<BehaviorFormSection
			step={1}
			title="เลือกวิธีจัดการ"
			description="รีเซ็ตเริ่มรอบใหม่ที่ศูนย์และเก็บยอดเดิมเป็นประวัติ หรือล้างรายการตามขอบเขต"
		>
			<ToggleGroup.Root
				type="single"
				bind:value={() => (reset ? 'reset' : 'cleanup'), chooseOperation}
				aria-label="วิธีจัดการคะแนน"
				class="grid grid-cols-2 gap-2"
				disabled={!!pending || !!requestId}
			>
				<ToggleGroup.Item
					value="reset"
					class="gap-2 data-[state=on]:border-primary data-[state=on]:bg-primary/5"
					><RotateCcw class="size-4 shrink-0" aria-hidden="true" />รีเซ็ตคะแนน</ToggleGroup.Item
				>
				<ToggleGroup.Item
					value="cleanup"
					class="gap-2 data-[state=on]:border-primary data-[state=on]:bg-primary/5"
					><Trash2 class="size-4 shrink-0" aria-hidden="true" />ล้างรายการ</ToggleGroup.Item
				>
			</ToggleGroup.Root>
			{#if reset}<p class="rounded-xl bg-muted/30 p-3 text-sm text-muted-foreground">
					รีเซ็ตฝั่งที่เลือกทั้งโรงเรียน ต้องปิดยอดเช็กชื่อถึงวันที่เลือกก่อน
				</p>{:else}
				<div class="space-y-2">
					<Label id="cleanup-mode-label">วิธีล้างข้อมูล</Label><ToggleGroup.Root
						type="single"
						bind:value={() => mode, chooseMode}
						aria-labelledby="cleanup-mode-label"
						class="flex-col items-stretch gap-2"
						disabled={!!pending || !!requestId}
					>
						{#each modes as option (option)}<ToggleGroup.Item
								value={option}
								class="h-auto justify-start whitespace-normal px-3 py-1.5 text-left data-[state=on]:border-primary data-[state=on]:bg-primary/5"
								>{modeLabels[option]}</ToggleGroup.Item
							>{/each}
					</ToggleGroup.Root>
				</div>
			{/if}
		</BehaviorFormSection>
		<BehaviorFormSection
			step={2}
			title="ขอบเขตข้อมูล"
			description="เลือกฝั่งคะแนนและช่วงวันที่ที่ต้องการจัดการ"
		>
			<div class="space-y-2">
				<p class="text-sm font-medium">ฝั่งคะแนน</p>
				<div class="grid gap-3 sm:grid-cols-2">
					{#each sides as side (side)}<label
							class="flex items-center gap-3 rounded-xl border p-3 has-[[data-state=checked]]:border-primary/30 has-[[data-state=checked]]:bg-primary/5"
							><Checkbox
								checked={chosenSides.includes(side)}
								onCheckedChange={(checked) =>
									(chosenSides = checked
										? [...chosenSides, side]
										: chosenSides.filter((candidate) => candidate !== side))}
							/>{sideLabels[side]}</label
						>{/each}
				</div>
			</div>
			{#if !reset}<div class="space-y-2 border-t pt-4">
					<label class="flex items-center gap-3"
						><Checkbox bind:checked={termOnly} />เฉพาะภาคเรียนที่เลือก (ปิดเพื่อเลือกทุกภาคเรียน)</label
					><label class="flex items-center gap-3"
						><Checkbox bind:checked={allDates} />ทุกวันในขอบเขตที่เลือก</label
					>
				</div>{/if}
			<div class="grid gap-4 sm:grid-cols-2">
				{#if !reset}<div class="space-y-2">
						<Label for="cleanup-start">เริ่มวันที่</Label><DatePicker
							id="cleanup-start"
							value={start}
							disabled={allDates}
							onValueChange={(value) => {
								if (value) start = value;
							}}
						/>
					</div>{/if}
				<div class="space-y-2">
					<Label for="cleanup-end">ถึงวันที่ *</Label><DatePicker
						id="cleanup-end"
						value={end}
						disabled={!reset && allDates}
						onValueChange={(value) => {
							if (value) end = value;
						}}
						required
					/>
				</div>
			</div>
			{#if !reset}<div class="space-y-2 border-t pt-4">
					<p class="text-sm font-medium">ที่มาของคะแนน</p>
					<div class="grid gap-2 sm:grid-cols-2">
						{#each sources as source (source)}<label class="flex items-center gap-3 rounded-lg px-2"
								><Checkbox
									checked={chosenSources.includes(source)}
									onCheckedChange={(checked) =>
										(chosenSources = checked
											? [...chosenSources, source]
											: chosenSources.filter((candidate) => candidate !== source))}
								/>{sourceLabels[source]}</label
							>{/each}
					</div>
				</div>{/if}
		</BehaviorFormSection>
	</fieldset>
	<div class="flex justify-end border-t pt-4">
		<LoadingButton
			type="submit"
			variant="outline"
			loading={pending === 'preview'}
			disabled={!!pending || !chosenSides.length || (!reset && !chosenSources.length)}
			><SearchCheck class="size-4" aria-hidden="true" />ตรวจผลกระทบก่อนดำเนินการ</LoadingButton
		>
	</div>
</form>
{#if error}<PageState variant="error" title="ดำเนินการไม่สำเร็จ" description={error} />{/if}
{#if preview}<div class="max-w-5xl">
		<BehaviorFormSection
			step={3}
			title="ผลกระทบที่ตรวจแล้ว"
			description="ตรวจยอดและจำนวนรายการก่อนยืนยันดำเนินการ"
		>
			<div class="grid gap-3 sm:grid-cols-2">
				<div class="rounded-xl border bg-muted/20 p-4">
					<p class="text-sm text-muted-foreground">นักเรียนที่ได้รับผลกระทบ</p>
					<p class="mt-2 text-2xl font-semibold tabular-nums">
						{preview.students} <span class="text-sm font-normal">คน</span>
					</p>
				</div>
				<div class="rounded-xl border bg-muted/20 p-4">
					<p class="text-sm text-muted-foreground">รายละเอียดในขอบเขต</p>
					<p class="mt-2 text-2xl font-semibold tabular-nums">
						{preview.entries} <span class="text-sm font-normal">รายการ</span>
					</p>
				</div>
			</div>
			<dl class="grid gap-3 sm:grid-cols-2">
				<div class="rounded-xl border border-success/20 bg-success/5 p-4">
					<dt class="text-sm text-muted-foreground">คะแนนความดีในขอบเขต</dt>
					<dd class="mt-1 text-xl font-semibold text-success">{preview.merit}</dd>
				</div>
				<div class="rounded-xl border border-destructive/20 bg-destructive/5 p-4">
					<dt class="text-sm text-muted-foreground">คะแนนที่ถูกหักในขอบเขต</dt>
					<dd class="mt-1 text-xl font-semibold text-destructive">{preview.deduction}</dd>
				</div>
			</dl>
			<div class="grid gap-3 sm:grid-cols-3">
				{#each [{ label: 'ปัจจุบัน', totals: preview.current }, { label: 'ประวัติ', totals: preview.history }, { label: 'สะสม', totals: preview.lifetime }] as group (group.label)}<div
						class="rounded-xl border p-3"
					>
						<h3 class="mb-2 text-sm font-medium">{group.label}</h3>
						<dl class="grid grid-cols-2 gap-2 text-sm">
							<dt class="text-muted-foreground">ความดี</dt>
							<dd class="text-end font-medium">{group.totals.merit}</dd>
							<dt class="text-muted-foreground">ถูกหัก</dt>
							<dd class="text-end font-medium">{group.totals.deduction}</dd>
						</dl>
					</div>{/each}
			</div>
			<div class="flex items-start gap-3 rounded-xl border border-warning/30 bg-warning/10 p-4">
				<TriangleAlert class="mt-0.5 size-4 shrink-0 text-warning" aria-hidden="true" />
				<p class="text-sm">
					รายละเอียดที่ลบแล้วจะเปิดดูหรือยกเลิกรายเหตุการณ์ไม่ได้
					ข้อมูลเช็กชื่อยังอยู่ในระบบเช็กชื่อ
				</p>
			</div>
			<div class="space-y-2">
				<Label for="cleanup-reason">เหตุผล *</Label><Textarea
					id="cleanup-reason"
					bind:value={reason}
					required
					maxlength={1000}
					disabled={!!pending || !!requestId}
				/>
			</div>
			{#if previewKey !== key}<p class="text-sm text-warning">
					ขอบเขตเปลี่ยน กรุณาตรวจผลกระทบใหม่
				</p>{/if}
			<div class="flex justify-end border-t pt-4">
				<Button
					type="button"
					variant="destructive"
					disabled={!!pending || previewKey !== key || !reason.trim()}
					onclick={() => (confirm = true)}>ดำเนินการตามผลที่ตรวจ</Button
				>
			</div>
		</BehaviorFormSection>
	</div>{/if}
{#if outcome}<Card.Root role="status" class="max-w-5xl gap-0 py-0"
		><Card.Content class="space-y-2 p-4 sm:p-5"
			><h2 class="font-semibold">
				{outcome.status === 'pending' ? 'กำลังจัดการข้อมูล' : 'จัดการข้อมูลสำเร็จ'}
			</h2>
			<p class="text-sm text-muted-foreground">
				ลบรายละเอียดแล้ว {outcome.entriesDeleted} รายการ · นักเรียน {outcome.students} คน
			</p>
			{#if outcome.status === 'pending' && !pending}<Button
					type="button"
					variant="outline"
					onclick={() => {
						pending = 'job';
						void poll(outcome!.id);
					}}>ตรวจสถานะต่อ</Button
				>{/if}</Card.Content
		></Card.Root
	>{/if}
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
