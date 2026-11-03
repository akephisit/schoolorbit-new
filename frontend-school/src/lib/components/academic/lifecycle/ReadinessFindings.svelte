<script lang="ts">
	import {
		groupReadinessFindings,
		type LifecycleFinding
	} from '#lib/academic/lifecycle/readiness-presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	let {
		findings,
		acknowledgementId = '',
		acknowledged = [],
		disabled = false,
		onacknowledged
	}: {
		findings: LifecycleFinding[];
		acknowledgementId?: string;
		acknowledged?: string[];
		disabled?: boolean;
		onacknowledged?: (codes: string[]) => void;
	} = $props();
	const groups = $derived(groupReadinessFindings(findings));
</script>

<div class="space-y-4" aria-label="รายการที่ต้องตรวจสอบ">
	{#each groups as group (group.key)}
		<section class="space-y-2" aria-label={group.label}>
			<h3 class="font-semibold">{group.label}</h3>
			{#each group.findings as finding, index (`${finding.code}:${index}`)}
				<div class="space-y-2 rounded-lg border p-3">
					<p class={finding.severity === 'blocking' ? 'text-sm text-destructive' : 'text-sm'}>
						{finding.message}
					</p>
					<p class="text-xs text-muted-foreground">
						{finding.severity === 'blocking' ? 'ต้องแก้ก่อน' : 'ควรตรวจสอบ'} · {finding.count} รายการ
					</p>
					{#if finding.resolutionUrl}<Button
							size="sm"
							variant="outline"
							href={finding.resolutionUrl}>ไปตรวจและแก้ข้อมูล</Button
						>{/if}
					{#if onacknowledged && finding.severity === 'warning'}
						<div class="flex items-center gap-2">
							<Checkbox
								id={`${acknowledgementId}-${group.key}-${index}`}
								checked={acknowledged.includes(finding.code)}
								{disabled}
								onCheckedChange={(checked) =>
									onacknowledged?.(
										checked
											? [...acknowledged, finding.code]
											: acknowledged.filter((code) => code !== finding.code)
									)}
							/><Label for={`${acknowledgementId}-${group.key}-${index}`}>รับทราบ</Label>
						</div>
					{/if}
				</div>
			{/each}
		</section>
	{/each}
</div>
