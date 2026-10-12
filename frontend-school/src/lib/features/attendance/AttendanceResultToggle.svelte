<script lang="ts">
	import { Check, Clock, X, CalendarDays, Users, Undo2 } from '@lucide/svelte';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { resultLabels, type AttendanceResult } from '#lib/api/attendance.js';
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
	const presentation = {
		present: {
			icon: Check,
			tone: 'data-[state=on]:border-success/50 data-[state=on]:bg-success/15 data-[state=on]:text-success dark:data-[state=on]:bg-success/15 dark:data-[state=on]:hover:bg-success/20'
		},
		late: {
			icon: Clock,
			tone: 'data-[state=on]:border-warning/50 data-[state=on]:bg-warning/15 data-[state=on]:text-warning dark:data-[state=on]:bg-warning/15 dark:data-[state=on]:hover:bg-warning/20'
		},
		absent: {
			icon: X,
			tone: 'data-[state=on]:border-destructive/50 data-[state=on]:bg-destructive/15 data-[state=on]:text-destructive dark:data-[state=on]:bg-destructive/15 dark:data-[state=on]:hover:bg-destructive/20'
		},
		leave: {
			icon: CalendarDays,
			tone: 'data-[state=on]:border-primary/50 data-[state=on]:bg-primary/15 data-[state=on]:text-primary dark:data-[state=on]:bg-primary/15 dark:data-[state=on]:hover:bg-primary/20'
		},
		activity: {
			icon: Users,
			tone: 'data-[state=on]:border-chart-4/50 data-[state=on]:bg-chart-4/15 data-[state=on]:text-foreground dark:data-[state=on]:bg-chart-4/15 dark:data-[state=on]:hover:bg-chart-4/20'
		}
	} satisfies Record<Exclude<AttendanceResult, 'unchecked'>, { icon: typeof Check; tone: string }>;
</script>

<div role="group" aria-label={label} class="flex min-w-0 items-center gap-1 max-[359px]:flex-wrap">
	<span class="sr-only" aria-live="polite">{resultLabels[value]}</span>
	<ToggleGroup.Root
		type="single"
		aria-label={`เลือก${label}`}
		{disabled}
		bind:value={
			() => (value === 'unchecked' ? '' : value),
			(next) => {
				if (Object.hasOwn(presentation, next)) onValueChange(next as AttendanceResult);
			}
		}
		class="grid min-w-0 flex-1 grid-cols-5 gap-1 max-[359px]:basis-full"
	>
		{#each Object.entries(presentation) as [result, item] (result)}
			<ToggleGroup.Item
				value={result}
				aria-label={resultLabels[result as AttendanceResult]}
				class={`h-[44px] min-w-[44px] gap-1 px-0 text-xs shadow-none xl:px-2 xl:text-sm ${value === result ? item.tone : 'text-muted-foreground'}`}
			>
				<item.icon aria-hidden="true" class="hidden size-4 xl:block" />
				{resultLabels[result as AttendanceResult]}
			</ToggleGroup.Item>
		{/each}
	</ToggleGroup.Root>
	{#if allowUnchecked}
		<Button
			type="button"
			variant="ghost"
			class="size-[44px] shrink-0 text-muted-foreground"
			disabled={disabled || value === 'unchecked'}
			aria-label={`ยกเลิกการเลือก${label}`}
			title="กลับเป็นยังไม่เช็ค"
			onclick={() => onValueChange('unchecked')}><Undo2 aria-hidden="true" class="size-4" /></Button
		>
	{/if}
</div>
