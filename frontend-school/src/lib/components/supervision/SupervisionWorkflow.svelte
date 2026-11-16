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
	class="rounded-lg border bg-card p-4"
	aria-label="ความคืบหน้าการนิเทศ"
	data-testid="supervision-workflow"
>
	<div class="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
		<p class="text-sm font-medium">{workflow.stageLabel}</p>
		<p class="text-sm">
			ถัดไป: <span class="font-medium">{workflow.nextOwner}</span> · {workflow.nextAction}
		</p>
	</div>
	{#if workflow.stage >= 0}
		<ol class="mt-3 grid grid-cols-3 gap-2 sm:grid-cols-6" aria-label="ขั้นตอนการนิเทศ">
			{#each supervisionStages as label, index (label)}
				<li
					class={cn(
						'rounded-md border px-2 py-2 text-center text-xs',
						index < workflow.stage && 'bg-muted',
						index === workflow.stage && 'border-primary bg-primary/10 font-medium'
					)}
					aria-current={index === workflow.stage ? 'step' : undefined}
				>
					{index + 1}. {label}
				</li>
			{/each}
		</ol>
	{/if}
	{#if workflow.stage === 1}
		<p class="mt-3 text-xs text-muted-foreground">{workflow.evaluatorProgress}</p>
		{#if workflow.pendingRequiredLabel}<p class="mt-1 text-xs text-muted-foreground">
				{workflow.pendingRequiredLabel}
			</p>{/if}
	{/if}
</section>
