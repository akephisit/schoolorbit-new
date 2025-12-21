<script lang="ts">
	import CalendarIcon from 'lucide-svelte/icons/calendar';
	import {
		type DateValue,
		DateFormatter,
		getLocalTimeZone,
		parseDate
	} from '@internationalized/date';
	import { cn } from '$lib/utils.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Calendar } from '$lib/components/ui/calendar/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';

	interface Props {
		value?: string; // ISO date string (YYYY-MM-DD)
		placeholder?: string;
		class?: string;
		onValueChange?: (value: string | undefined) => void;
	}

	let {
		value = $bindable(),
		placeholder = 'เลือกวันที่',
		class: className = '',
		onValueChange
	}: Props = $props();

	const df = new DateFormatter('th-TH', {
		dateStyle: 'long'
	});

	// Convert string to DateValue
	let dateValue = $derived<DateValue | undefined>(value ? parseDate(value) : undefined);

	// Handle calendar value change
	function handleValueChange(newValue: DateValue | undefined) {
		const isoString = newValue ? newValue.toString() : undefined;
		value = isoString;
		onValueChange?.(isoString);
	}
</script>

<Popover.Root>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				variant="outline"
				class={cn(
					'w-full justify-start text-start font-normal',
					!value && 'text-muted-foreground',
					className
				)}
				{...props}
			>
				<CalendarIcon class="me-2 size-4" />
				{value && dateValue ? df.format(dateValue.toDate(getLocalTimeZone())) : placeholder}
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-auto p-0">
		<Calendar
			value={dateValue}
			onValueChange={handleValueChange}
			type="single"
			initialFocus
			captionLayout="dropdown"
			locale="th-TH"
		/>
	</Popover.Content>
</Popover.Root>
