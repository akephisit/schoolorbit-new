<script lang="ts">
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Undo2 } from '@lucide/svelte';
	import {
		resultLabels,
		writableAttendanceResults,
		type AttendanceResult
	} from '#lib/api/attendance.js';
	let {
		value,
		label,
		allowUnchecked,
		disabled = false,
		onValueChange
	}: {
		value: AttendanceResult;
		label: string;
		allowUnchecked: boolean;
		disabled?: boolean;
		onValueChange: (value: AttendanceResult) => void;
	} = $props();
	function choose(next: string) {
		const result = writableAttendanceResults.find((r) => r === next);
		if (result) onValueChange(result);
	}
	function tone(result: AttendanceResult) {
		if (result === 'present')
			return 'data-[state=on]:bg-success/15 dark:data-[state=on]:bg-success/15 dark:data-[state=on]:hover:bg-success/20 data-[state=on]:text-success data-[state=on]:border-success/50';
		if (result === 'absent' || result === 'skipped')
			return 'data-[state=on]:bg-destructive/15 dark:data-[state=on]:bg-destructive/15 dark:data-[state=on]:hover:bg-destructive/20 data-[state=on]:text-destructive data-[state=on]:border-destructive/50';
		if (result === 'activity')
			return 'data-[state=on]:bg-chart-4/15 dark:data-[state=on]:bg-chart-4/15 dark:data-[state=on]:hover:bg-chart-4/20 data-[state=on]:text-foreground data-[state=on]:border-chart-4/50';
		if (result === 'late')
			return 'data-[state=on]:bg-warning/15 dark:data-[state=on]:bg-warning/15 dark:data-[state=on]:hover:bg-warning/20 data-[state=on]:text-warning data-[state=on]:border-warning/50';
		return 'data-[state=on]:bg-primary/15 dark:data-[state=on]:bg-primary/15 dark:data-[state=on]:hover:bg-primary/20 data-[state=on]:text-primary data-[state=on]:border-primary/50';
	}
</script>

<div role="group" aria-label={label} class="flex min-w-0 items-start gap-1">
	<span class="sr-only" aria-live="polite">{resultLabels[value]}</span><ToggleGroup.Root
		type="single"
		aria-label={`เลือก${label}`}
		{disabled}
		bind:value={() => (value === 'unchecked' ? '' : value), choose}
		class="grid min-w-0 flex-1 grid-cols-4 gap-1 xl:grid-cols-7"
		>{#each writableAttendanceResults as result (result)}<ToggleGroup.Item
				value={result}
				aria-label={resultLabels[result]}
				class={`min-h-[44px] min-w-[44px] px-1 text-xs ${tone(result)}`}
				>{resultLabels[result]}</ToggleGroup.Item
			>{/each}</ToggleGroup.Root
	>{#if allowUnchecked}<Button
			type="button"
			variant="ghost"
			class="size-[44px] shrink-0"
			disabled={disabled || value === 'unchecked'}
			aria-label={`ยกเลิกการเลือก${label}`}
			title="กลับเป็นยังไม่ได้เช็ค"
			onclick={() => onValueChange('unchecked')}><Undo2 class="size-4" /></Button
		>{/if}
</div>
