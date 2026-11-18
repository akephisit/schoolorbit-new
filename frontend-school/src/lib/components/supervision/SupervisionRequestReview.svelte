<script lang="ts">
	import { Check, ChevronsUpDown, Eye, Trash2 } from '@lucide/svelte';
	import type { SupervisionObservation } from '#lib/api/supervision.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import SupervisionRequestAvailability from '#lib/components/supervision/SupervisionRequestAvailability.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import type { SupervisionRequestReview } from '#lib/supervision/request-review.svelte.js';
	import {
		savedScheduleDateLabel,
		savedScheduleSummary
	} from '#lib/supervision/supervision-schedule.js';
	import { cn } from '#lib/utils.js';

	let {
		observation,
		review,
		detailHref,
		showSaveEvaluators = false,
		showSchedule = false,
		onclose
	}: {
		observation: SupervisionObservation;
		review: SupervisionRequestReview;
		detailHref?: string;
		showSaveEvaluators?: boolean;
		showSchedule?: boolean;
		onclose?: () => void;
	} = $props();

	const fieldId = $props.id();
	const id = $derived(observation.id);
	const schedule = $derived(showSchedule ? savedScheduleSummary(observation.periods) : undefined);
	const canReview = $derived(review.canReview(observation));
	const canChange = $derived(review.canChange(observation));
	const disabled = $derived(!canChange || review.isBusy());
	const state = $derived(review.state(observation));
	const selected = $derived(review.selectedEvaluators(observation));
	const available = $derived(state?.availability?.items.filter((item) => item.available) ?? []);
	const unavailable = $derived(state?.availability?.items.filter((item) => !item.available) ?? []);
	const selectionUnverified = $derived(Boolean(state?.error || state?.loading));
	let pickerOpen = $state(false);

	function setPickerOpen(open: boolean) {
		pickerOpen = open && !disabled;
		if (pickerOpen) void review.loadAvailability(id);
	}

	function formatConflictDate(value: string) {
		return new Intl.DateTimeFormat('th-TH', {
			timeZone: 'Asia/Bangkok',
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
	}
</script>

<div class="space-y-3" data-testid="supervision-request-review">
	{#if schedule}
		<div class="space-y-1 rounded-lg border border-primary/15 bg-primary/5 p-3 text-sm">
			<p class="font-semibold text-primary">{schedule.periodTitle}</p>
			<p>{schedule.dates.map(savedScheduleDateLabel).join(' · ') || 'ไม่ระบุวันนิเทศ'}</p>
			{#if schedule.timeLabel}
				<p class="tabular-nums">{schedule.timeLabel}</p>
			{:else if schedule.periods.length}
				<ul class="space-y-1 tabular-nums">
					{#each schedule.periods as period (period.id)}
						<li>
							{#if schedule.dates.length !== 1}
								{period.date ? savedScheduleDateLabel(period.date) : 'ไม่ระบุวันนิเทศ'} ·
							{/if}
							{period.label ?? 'ไม่ระบุคาบ'} · {period.timeLabel}
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
	{#if canReview}
		{#if state?.needsRecheck}
			<p class="text-sm text-muted-foreground">
				ช่วงคาบหรือรอบนิเทศเปลี่ยนแล้ว กรุณาตรวจช่วงคาบและผู้ประเมินอีกครั้ง
			</p>
		{/if}
		<SupervisionRequestAvailability
			result={state?.availability}
			loading={Boolean(state?.loading)}
			error={state?.error ?? ''}
			showError={!pickerOpen}
			{disabled}
			oninspect={() => void review.loadAvailability(id)}
		/>
	{/if}
	<div class="grid gap-4 border-t pt-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
		<div class="min-w-0 space-y-2">
			<Label for={`${fieldId}-evaluators`}>ผู้ประเมิน</Label>
			<div class="flex min-h-10 flex-wrap items-center gap-2 rounded-md border p-2">
				{#if selected.length === 0}
					<span class="text-sm text-muted-foreground">ยังไม่ได้เลือกผู้ประเมิน</span>
				{:else}
					{#each selected as evaluator (evaluator.id)}
						<Badge
							variant={evaluator.blocked ? 'outline' : 'secondary'}
							class="max-w-full gap-1 pr-1 whitespace-normal"
						>
							<span class="min-w-0 break-words"
								>{evaluator.name}{evaluator.blocked ? ' · ไม่ว่าง' : ''}</span
							>
							{#if canChange}
								<Button
									size="icon-sm"
									variant="ghost"
									class="size-6"
									aria-label={`ลบผู้ประเมิน ${evaluator.name}`}
									{disabled}
									onclick={() => review.removeEvaluator(id, evaluator.id)}
									><Trash2 class="size-3" /></Button
								>
							{/if}
						</Badge>
					{/each}
				{/if}
			</div>
			{#if selected.some((evaluator) => evaluator.blocked)}
				<p class="text-sm text-destructive">
					นำผู้ประเมินที่ไม่ว่างออกหรือเลือกคนอื่นก่อนบันทึกหรืออนุมัติ
				</p>
			{/if}
			<Popover.Root open={pickerOpen} onOpenChange={setPickerOpen}>
				<Popover.Trigger disabled={disabled || state?.loading}>
					{#snippet child({ props })}
						<Button
							type="button"
							variant="outline"
							role="combobox"
							aria-label="เพิ่ม/เลือกผู้ประเมิน"
							aria-expanded={pickerOpen}
							class="w-full justify-between font-normal"
							{...props}
							id={`${fieldId}-evaluators`}
						>
							<span class="truncate">
								{state?.loading && !state.availability
									? 'กำลังตรวจผู้ประเมินที่ว่าง...'
									: 'เพิ่ม/เลือกผู้ประเมิน'}
							</span>
							<ChevronsUpDown class="ml-2 size-4 shrink-0 opacity-50" />
						</Button>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content class="w-(--bits-popover-trigger-width) p-0">
					{#if state?.loading && !state.availability}
						<div role="status" aria-label="กำลังโหลดข้อมูล"><PageSkeleton variant="table" /></div>
					{/if}
					{#if state?.error}
						<PageState
							variant="error"
							title="โหลดข้อมูลไม่สำเร็จ"
							description={state.error}
							actionLabel="ลองใหม่"
							onaction={() => void review.loadAvailability(id)}
						/>
					{/if}
					<Command.Root>
						<Command.Input placeholder="ค้นหาครูผู้ประเมิน..." />
						{#if state?.availability}
							<Command.Empty>ไม่พบครูผู้ประเมินที่ว่าง</Command.Empty>
							<Command.List class="max-h-72">
								<Command.Group heading="พร้อมประเมินตลอดช่วงคาบ">
									{#each available as evaluator (evaluator.id)}
										<Command.Item
											value={`${evaluator.name} ${evaluator.title ?? ''} ${evaluator.id}`}
											disabled={disabled || selectionUnverified}
											onSelect={() => review.toggleEvaluator(id, evaluator)}
										>
											<Check
												class={cn(
													'mr-2 size-4',
													selected.some((item) => item.id === evaluator.id)
														? 'opacity-100'
														: 'opacity-0'
												)}
											/>
											<span>{evaluator.name}</span>
										</Command.Item>
									{/each}
								</Command.Group>
								{#if unavailable.length}
									<Command.Group heading="ไม่ว่างในช่วงคาบนี้">
										{#each unavailable as evaluator (evaluator.id)}
											<Command.Item value={`${evaluator.name} ${evaluator.id}`} disabled>
												<div class="min-w-0 space-y-1">
													<p>{evaluator.name}</p>
													<p class="text-xs text-muted-foreground">
														{evaluator.conflictReason ?? 'มีงานทับช่วงคาบ'}
													</p>
													{#if evaluator.conflict}
														<p class="text-xs text-muted-foreground">
															{evaluator.conflict.observedDisplayName ?? 'รายการนิเทศ'} · {formatConflictDate(
																evaluator.conflict.observedAt
															)}
															{evaluator.conflict.lessonTitle
																? ` · ${evaluator.conflict.lessonTitle}`
																: ''}
														</p>
													{/if}
												</div>
											</Command.Item>
										{/each}
									</Command.Group>
								{/if}
							</Command.List>
						{/if}
					</Command.Root>
				</Popover.Content>
			</Popover.Root>
			{#if unavailable.length}
				<p class="text-xs text-muted-foreground">
					ผู้ประเมินไม่ว่าง {unavailable.length} คน ดูเหตุผลในกลุ่ม “ไม่ว่างในช่วงคาบนี้”
				</p>
			{/if}
		</div>
		<div class="min-w-0 space-y-2">
			<Label for={`${fieldId}-comment`}>ส่งกลับคำขอ</Label>
			<Textarea
				id={`${fieldId}-comment`}
				value={state?.comment ?? ''}
				{disabled}
				rows={3}
				placeholder="ระบุเหตุผลส่งกลับ"
				oninput={(event) => review.setComment(id, event.currentTarget.value)}
			/>
		</div>
	</div>

	<div class="flex flex-wrap items-center justify-end gap-2">
		{#if onclose}<Button variant="outline" onclick={onclose}>ปิด</Button>{/if}
		{#if detailHref}
			<Button size="sm" variant="outline" href={detailHref} data-sveltekit-preload-data="tap">
				<Eye class="size-4" />รายละเอียด
			</Button>
		{/if}
		{#if showSaveEvaluators}
			<LoadingButton
				variant="outline"
				disabled={!review.canSubmit(observation, 'save-evaluators')}
				loading={review.isSaving(id, 'save-evaluators')}
				loadingLabel="กำลังบันทึก..."
				onclick={() => review.submit(id, 'save-evaluators')}>บันทึกผู้ประเมิน</LoadingButton
			>
		{/if}
		<LoadingButton
			variant="outline"
			disabled={!review.canSubmit(observation, 'return-request')}
			loading={review.isSaving(id, 'return-request')}
			loadingLabel="กำลังส่งกลับ..."
			onclick={() => review.submit(id, 'return-request')}>ส่งกลับคำขอ</LoadingButton
		>
		<LoadingButton
			disabled={!review.canSubmit(observation, 'approve-request')}
			loading={review.isSaving(id, 'approve-request')}
			loadingLabel="กำลังอนุมัติ..."
			onclick={() => review.submit(id, 'approve-request')}>อนุมัติและมอบหมาย</LoadingButton
		>
	</div>
</div>
