<script lang="ts">
	import type { SupervisionObservation } from '#lib/api/supervision.js';
	import {
		observationWorkflow,
		supervisionStages
	} from '#lib/supervision/supervision-presentation.js';
	import { cn } from '#lib/utils.js';
	let { observation }: { observation: SupervisionObservation } = $props();
	const workflow = $derived(observationWorkflow(observation));
	const awaitingAction = $derived(
		workflow.stage >= 0 && workflow.stage < supervisionStages.length - 1
	);
</script>

<section
	class="space-y-3 rounded-xl border bg-card p-4 sm:px-5"
	aria-label="ความคืบหน้าการนิเทศ"
	data-testid="supervision-workflow"
>
	<div class="flex flex-wrap items-baseline justify-between gap-x-5 gap-y-1.5">
		<h2 class="text-sm font-semibold tabular-nums">
			{workflow.stage >= 0 ? workflow.stageLabel : workflow.nextAction}
		</h2>
		{#if workflow.stage === 1}
			<p class="text-xs tabular-nums text-muted-foreground">{workflow.evaluatorProgress}</p>
		{/if}
	</div>
	{#if workflow.stage >= 0}
		<ol class="grid grid-cols-2 gap-x-3 gap-y-2 sm:grid-cols-4" aria-label="ขั้นตอนการนิเทศ">
			{#each supervisionStages as label, index (label)}
				<li
					class={cn(
						'flex min-w-0 items-center gap-2 border-t-2 py-1.5 text-sm',
						index < workflow.stage && 'border-primary/30 text-foreground',
						index === workflow.stage && 'border-primary font-semibold text-primary',
						index > workflow.stage && 'border-muted text-muted-foreground'
					)}
					aria-current={index === workflow.stage ? 'step' : undefined}
				>
					<span class="text-xs tabular-nums" aria-hidden="true">{index + 1}.</span>
					<span>{label}</span>
				</li>
			{/each}
		</ol>
		<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1 border-t pt-3 text-sm">
			{#if awaitingAction}<span class="text-muted-foreground">ถัดไป</span>{/if}
			<p class="font-medium wrap-anywhere">{workflow.nextAction}</p>
			{#if awaitingAction}<p class="wrap-anywhere text-muted-foreground">
					{workflow.nextOwner}
				</p>{/if}
		</div>
	{/if}
</section>
