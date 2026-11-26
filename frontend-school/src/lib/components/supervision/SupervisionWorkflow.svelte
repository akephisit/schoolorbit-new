<script lang="ts">
	import type { SupervisionObservation } from '#lib/api/supervision.js';
	import {
		observationWorkflow,
		supervisionStages
	} from '#lib/supervision/supervision-presentation.js';
	import { cn } from '#lib/utils.js';
	let { observation }: { observation: SupervisionObservation } = $props();
	const workflow = $derived(observationWorkflow(observation));
</script>

<section
	class="space-y-4 rounded-xl border bg-card p-4 sm:p-5"
	aria-label="ความคืบหน้าการนิเทศ"
	data-testid="supervision-workflow"
>
	<div class="flex flex-wrap items-start justify-between gap-x-6 gap-y-2">
		<div class="min-w-0 space-y-1">
			<h2 class="text-sm font-semibold text-muted-foreground">
				{workflow.stage >= 0 && workflow.stage < supervisionStages.length - 1
					? 'ขั้นตอนถัดไป'
					: 'ความคืบหน้า'}
			</h2>
			<p class="font-semibold wrap-anywhere">{workflow.nextAction}</p>
			<p class="text-sm wrap-anywhere text-muted-foreground">{workflow.nextOwner}</p>
		</div>
		{#if workflow.stage >= 0}
			<div class="space-y-1 sm:text-end">
				<p class="text-sm font-medium tabular-nums">{workflow.stageLabel}</p>
				{#if workflow.stage === 1}<p class="text-sm tabular-nums text-muted-foreground">
						{workflow.evaluatorProgress}
					</p>{/if}
			</div>
		{/if}
	</div>
	{#if workflow.stage >= 0}
		<ol class="grid grid-cols-2 gap-2 sm:grid-cols-4" aria-label="ขั้นตอนการนิเทศ">
			{#each supervisionStages as label, index (label)}
				<li
					class={cn(
						'flex min-w-0 items-center gap-2 rounded-md border px-2.5 py-2 text-sm',
						index < workflow.stage && 'border-transparent bg-muted/70 text-muted-foreground',
						index === workflow.stage &&
							'border-primary/40 bg-primary/10 font-semibold text-primary',
						index > workflow.stage && 'text-muted-foreground'
					)}
					aria-current={index === workflow.stage ? 'step' : undefined}
				>
					<span
						class="flex size-6 shrink-0 items-center justify-center rounded-full bg-background text-xs tabular-nums"
						aria-hidden="true">{index + 1}</span
					>
					<span>{label}</span>
				</li>
			{/each}
		</ol>
	{/if}
</section>
