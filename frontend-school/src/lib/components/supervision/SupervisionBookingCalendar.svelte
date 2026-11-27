<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { SupervisionCycle, SupervisionObservation } from '#lib/api/supervision.js';
	import SupervisionCalendarSchedule from '#lib/components/supervision/SupervisionCalendarSchedule.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { savedLessonLabels } from '#lib/supervision/supervision-presentation.js';
	import { savedPeriodLabel } from '#lib/supervision/supervision-schedule.js';

	let {
		observations,
		cycles,
		academicLabel,
		selectedDate = $bindable(''),
		detailHref,
		hasSubmittedEvaluation,
		bookingAction
	}: {
		observations: SupervisionObservation[];
		cycles: SupervisionCycle[];
		academicLabel: string;
		selectedDate?: string;
		detailHref: (observation: SupervisionObservation) => string;
		hasSubmittedEvaluation?: (observation: SupervisionObservation) => boolean;
		bookingAction?: Snippet<[SupervisionObservation]>;
	} = $props();

	const bookings = $derived(
		observations
			.filter(({ status }) => status !== 'returned' && status !== 'cancelled')
			.map((observation) => ({
				id: observation.id,
				cycleId: observation.cycleId,
				observedDisplayName: observation.observedDisplayName,
				status: observation.status,
				evaluatorDisplayNames: observation.evaluators.map(
					({ evaluatorDisplayName }) => evaluatorDisplayName ?? ''
				),
				periods: observation.periods.map((period) => ({
					...period,
					periodLabel: savedPeriodLabel(period)
				})),
				observation
			}))
	);
</script>

<SupervisionCalendarSchedule
	view={hasSubmittedEvaluation ? 'evaluate' : 'requests'}
	{bookings}
	{cycles}
	{academicLabel}
	bind:selectedDate
	submitted={(booking) => hasSubmittedEvaluation?.(booking.observation) ?? false}
>
	{#snippet bookingNotes(_, periods)}
		{@const lesson = savedLessonLabels({ periods })}
		<p class="mt-1 break-words text-muted-foreground">
			{lesson.subject} · {lesson.homeroom}{lesson.room === '—' ? '' : ` · ${lesson.room}`}
		</p>
	{/snippet}
	{#snippet bookingActions({ observation })}
		<Button
			variant="ghost"
			size="sm"
			href={detailHref(observation)}
			data-sveltekit-preload-data="tap"
			aria-label={`รายละเอียด ${observation.observedDisplayName ?? 'รายการนิเทศ'}`}
		>
			รายละเอียด
		</Button>
		{@render bookingAction?.(observation)}
	{/snippet}
</SupervisionCalendarSchedule>
