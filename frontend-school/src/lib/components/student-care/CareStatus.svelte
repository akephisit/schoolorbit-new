<script lang="ts">
	import {
		Clock3,
		CircleCheck,
		CircleX,
		PencilLine,
		Eye,
		HeartHandshake,
		ClipboardList,
		LockKeyhole
	} from '@lucide/svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import type {
		CareAssignmentStatus,
		CareRoundStatus,
		CareVisitStatus,
		CareCaseStatus
	} from '#lib/api/student-care.js';
	import {
		assignmentLabels,
		roundLabels,
		visitLabels,
		caseLabels
	} from '#lib/features/student-care/presentation.js';
	type Status =
		| { kind: 'assignment'; value: CareAssignmentStatus }
		| { kind: 'round'; value: CareRoundStatus }
		| { kind: 'visit'; value: CareVisitStatus }
		| { kind: 'case'; value: CareCaseStatus };
	let { status }: { status: Status } = $props();
	const label = $derived(
		status.kind === 'assignment'
			? assignmentLabels[status.value]
			: status.kind === 'round'
				? roundLabels[status.value]
				: status.kind === 'visit'
					? visitLabels[status.value]
					: caseLabels[status.value]
	);
	const appearance = $derived.by(() => {
		if (status.value === 'cancelled') return { icon: CircleX, class: 'text-muted-foreground' };
		if (status.kind === 'round')
			return status.value === 'closed'
				? { icon: LockKeyhole, class: 'text-muted-foreground' }
				: { icon: ClipboardList, class: 'border-primary/20 bg-primary/5 text-primary' };
		if (status.kind === 'case')
			return status.value === 'closed'
				? { icon: CircleCheck, class: 'border-success/30 bg-success/5 text-success' }
				: status.value === 'monitoring'
					? { icon: Eye, class: 'border-primary/20 bg-primary/5 text-primary' }
					: { icon: HeartHandshake, class: '' };
		if (status.value === 'submitted' || status.value === 'completed')
			return { icon: CircleCheck, class: 'border-success/30 bg-success/5 text-success' };
		if (status.value === 'in_progress')
			return { icon: PencilLine, class: 'border-primary/20 bg-primary/5 text-primary' };
		return { icon: Clock3, class: '' };
	});
</script>

<Badge variant="outline" class={appearance.class}
	><appearance.icon aria-hidden="true" />{label}</Badge
>
