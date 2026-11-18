<script lang="ts">
	import { ChevronDown } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import type { SupervisionObservation } from '#lib/api/supervision.js';
	import SupervisionSavedPeriods from '#lib/components/supervision/SupervisionSavedPeriods.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import {
		observationWorkflow,
		savedLessonLabels,
		supervisionStatusLabel
	} from '#lib/supervision/supervision-presentation.js';
	import {
		savedScheduleDateLabel,
		savedScheduleSummary
	} from '#lib/supervision/supervision-schedule.js';

	let {
		observation,
		cycleLabel,
		templateLabel,
		requestedLabel,
		children
	}: {
		observation: SupervisionObservation;
		cycleLabel: string;
		templateLabel: string;
		requestedLabel: string;
		children: Snippet;
	} = $props();

	const schedule = $derived(savedScheduleSummary(observation.periods));
	const lesson = $derived(savedLessonLabels(observation));
	const workflow = $derived(observationWorkflow(observation));
</script>

<Card.Root class="min-w-0 gap-3 p-4">
	<div class="flex flex-wrap items-start justify-between gap-2">
		<h3 class="min-w-0 font-semibold break-words">
			{observation.observedDisplayName ?? 'ครูผู้ขอรับนิเทศ'}
		</h3>
		<Badge variant="secondary">{supervisionStatusLabel(observation.status)}</Badge>
	</div>

	<div
		class="grid gap-2 rounded-lg border border-primary/15 bg-primary/5 p-3 sm:grid-cols-[1fr_auto]"
	>
		<div class="min-w-0">
			<p class="text-xl font-semibold break-words text-primary">{schedule.periodTitle}</p>
			<p class="mt-1 text-sm">
				{schedule.dates.map(savedScheduleDateLabel).join(' · ') || 'ไม่ระบุวันนิเทศ'}
				{#if schedule.timeLabel}
					<span class="font-medium tabular-nums"> · {schedule.timeLabel}</span>
				{/if}
			</p>
		</div>
		<div class="min-w-0 text-sm break-words sm:text-right">
			<p class="font-medium">
				{lesson.homeroom}{#if lesson.room !== '—'}
					· {lesson.room}{/if}
			</p>
			<p class="text-muted-foreground">{lesson.subject}</p>
		</div>
	</div>

	<Collapsible.Root>
		<Collapsible.Trigger>
			{#snippet child({ props })}
				<Button {...props} variant="ghost" size="sm" class="group">
					<ChevronDown class="size-4 transition-transform group-data-[state=open]:rotate-180" />
					{schedule.periods.length ? `ดูรายละเอียด ${schedule.periods.length} คาบ` : 'ดูข้อมูลคำขอ'}
				</Button>
			{/snippet}
		</Collapsible.Trigger>
		<Collapsible.Content class="space-y-3 pt-3 pb-1">
			{#if schedule.periods.length}
				<SupervisionSavedPeriods {observation} />
			{/if}
			<dl class="grid gap-2 text-sm sm:grid-cols-2">
				<div>
					<dt class="text-muted-foreground">รอบนิเทศ</dt>
					<dd>{cycleLabel}</dd>
				</div>
				<div>
					<dt class="text-muted-foreground">แบบประเมิน</dt>
					<dd>{templateLabel}</dd>
				</div>
				<div>
					<dt class="text-muted-foreground">ส่งคำขอ</dt>
					<dd>{requestedLabel}</dd>
				</div>
				<div>
					<dt class="text-muted-foreground">{workflow.stageLabel}</dt>
					<dd>ถัดไป: {workflow.nextOwner}</dd>
				</div>
			</dl>
		</Collapsible.Content>
	</Collapsible.Root>

	{@render children()}
</Card.Root>
