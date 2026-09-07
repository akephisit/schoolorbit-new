<script lang="ts">
	import { presentLearnerEvaluationDomain } from '$lib/academic/learner-evaluation/presentation';
	import type { StudentLearnerEvaluationSummary } from '$lib/api/academicLearnerEvaluations';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import { BookOpenCheck, CircleAlert, ShieldCheck } from 'lucide-svelte';

	let {
		summary,
		subjectLabels = {}
	}: {
		summary: StudentLearnerEvaluationSummary;
		subjectLabels?: Record<string, string>;
	} = $props();

	function domainLabel(domain: string): string {
		return domain === 'desirable_characteristic'
			? 'คุณลักษณะอันพึงประสงค์'
			: 'การอ่าน คิดวิเคราะห์ และเขียน';
	}
</script>

<div class="grid gap-4 lg:grid-cols-2">
	{#each summary.domains as domain (domain.domain)}
		{@const view = presentLearnerEvaluationDomain(domain, subjectLabels)}
		<Card.Root class="gap-4 py-4">
			<Card.Header class="px-4">
				<div class="flex items-start gap-3">
					<div
						class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary"
					>
						{#if domain.domain === 'desirable_characteristic'}
							<ShieldCheck class="size-5" />
						{:else}
							<BookOpenCheck class="size-5" />
						{/if}
					</div>
					<div class="min-w-0 flex-1">
						<Card.Title class="text-base">{domainLabel(domain.domain)}</Card.Title>
						<Card.Description>เฉลี่ยจากรายวิชาที่ล็อกผลแล้ว</Card.Description>
					</div>
					<Badge variant={view.status === 'complete' ? 'secondary' : 'outline'}>
						{view.statusLabel}
					</Badge>
				</div>
			</Card.Header>
			<Card.Content class="space-y-4 px-4">
				<div class="grid grid-cols-2 gap-3">
					<div class="rounded-lg border bg-muted/20 p-3">
						<p class="text-xs text-muted-foreground">ค่าเฉลี่ย</p>
						<p class="mt-1 text-2xl font-semibold tabular-nums">{view.averageLabel}</p>
					</div>
					<div class="rounded-lg border bg-muted/20 p-3">
						<p class="text-xs text-muted-foreground">ระดับคุณภาพ</p>
						<p class="mt-1 text-xl font-semibold">{view.qualityLabel}</p>
					</div>
				</div>
				{#if view.reasons.length > 0}
					<div class="rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900">
						<p class="flex items-center gap-2 font-medium">
							<CircleAlert class="size-4" /> ยังสรุปไม่ครบ
						</p>
						<ul class="mt-2 list-disc space-y-1 pl-5">
							{#each view.reasons as reason (reason)}<li>{reason}</li>{/each}
						</ul>
					</div>
				{/if}
			</Card.Content>
		</Card.Root>
	{/each}
</div>
