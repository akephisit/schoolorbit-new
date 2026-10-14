<script lang="ts">
	import type { SupplyProposal } from '#lib/api/supply-catalog.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { CircleCheck, CircleX, Clock, CircleQuestionMark } from '@lucide/svelte';
	let { status }: { status: SupplyProposal['status'] } = $props();
	const presentation = $derived(
		status === 'approved'
			? {
					label: 'อนุมัติแล้ว',
					icon: CircleCheck,
					className: 'border-success/30 bg-success/10 text-success'
				}
			: status === 'rejected'
				? {
						label: 'ไม่อนุมัติ',
						icon: CircleX,
						className: 'border-destructive/30 bg-destructive/10 text-destructive'
					}
				: status === 'pending'
					? {
							label: 'รออนุมัติ',
							icon: Clock,
							className: 'border-warning/30 bg-warning/10 text-warning'
						}
					: { label: 'ไม่ทราบสถานะ', icon: CircleQuestionMark, className: 'text-muted-foreground' }
	);
</script>

<Badge variant="outline" class={presentation.className}
	><presentation.icon aria-hidden="true" />{presentation.label}</Badge
>
