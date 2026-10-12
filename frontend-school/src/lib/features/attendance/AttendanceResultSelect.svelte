<script lang="ts">
	import { Check, Clock, X, CalendarDays, Users, Minus } from '@lucide/svelte';
	import * as Select from '#lib/components/ui/select/index.js';
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
		unchecked: {
			icon: Minus,
			tone: 'border-border bg-muted text-muted-foreground dark:bg-muted dark:hover:bg-muted'
		},
		present: {
			icon: Check,
			tone: 'border-success/40 bg-success/10 dark:bg-success/10 dark:hover:bg-success/15 text-success'
		},
		late: {
			icon: Clock,
			tone: 'border-warning/40 bg-warning/10 dark:bg-warning/10 dark:hover:bg-warning/15 text-warning'
		},
		absent: {
			icon: X,
			tone: 'border-destructive/40 bg-destructive/10 dark:bg-destructive/10 dark:hover:bg-destructive/15 text-destructive'
		},
		leave: {
			icon: CalendarDays,
			tone: 'border-primary/40 bg-primary/10 dark:bg-primary/10 dark:hover:bg-primary/15 text-primary'
		},
		activity: {
			icon: Users,
			tone: 'border-chart-4/40 bg-chart-4/10 dark:bg-chart-4/10 dark:hover:bg-chart-4/15 text-foreground'
		}
	} satisfies Record<AttendanceResult, { icon: typeof Check; tone: string }>;
	const current = $derived(presentation[value]);
</script>

<Select.Root
	type="single"
	{value}
	{disabled}
	onValueChange={(next) => {
		if (next in presentation) onValueChange(next as AttendanceResult);
	}}
>
	<Select.Trigger aria-label={label} class={`w-full min-w-0 ${current.tone}`}>
		<span class="flex min-w-0 items-center gap-1.5"
			><current.icon aria-hidden="true" class="size-4 text-current" /><span
				class={value === 'unchecked' ? 'text-muted-foreground' : 'text-foreground'}
				>{resultLabels[value]}</span
			></span
		>
	</Select.Trigger>
	<Select.Content>
		{#each Object.entries(resultLabels) as [result, text] (result)}
			{#if result !== 'unchecked' || allowUnchecked}
				{@const item = presentation[result as AttendanceResult]}
				<Select.Item value={result} label={text}>
					<span class={`flex items-center gap-2 rounded px-1.5 py-0.5 ${item.tone}`}
						><item.icon aria-hidden="true" class="size-4 text-current" /><span
							class={result === 'unchecked' ? 'text-muted-foreground' : 'text-foreground'}
							>{text}</span
						></span
					>
				</Select.Item>
			{/if}
		{/each}
	</Select.Content>
</Select.Root>
