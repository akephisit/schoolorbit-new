<script lang="ts">
	import type { CreditCriterionAssessment } from '#lib/academic-core/curriculum-credit-criteria.js';
	import { TriangleAlert } from '@lucide/svelte';
	import * as Tooltip from '#lib/components/ui/tooltip/index.js';
	let { assessment }: { assessment: CreditCriterionAssessment } = $props();
	let open = $state(false);
</script>

<Tooltip.Provider>
	<Tooltip.Root bind:open disableCloseOnTriggerClick>
		<Tooltip.Trigger
			type="button"
			aria-label={assessment.message}
			data-credit-status={assessment.status}
			class={`ml-1 inline-flex rounded-sm align-middle focus-visible:outline-2 focus-visible:outline-ring ${assessment.status === 'unknown' ? 'text-warning' : 'text-destructive'}`}
			onclick={() => (open = true)}
			><TriangleAlert class="size-4" aria-hidden="true" /></Tooltip.Trigger
		>
		<Tooltip.Content role="tooltip" class="max-w-80">{assessment.message}</Tooltip.Content>
	</Tooltip.Root>
</Tooltip.Provider>
