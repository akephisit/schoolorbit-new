<script lang="ts">
	import CalendarIcon from 'lucide-svelte/icons/calendar';
	import { type DateValue, getLocalTimeZone, parseDate } from '@internationalized/date';
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

	// Convert string to DateValue
	let dateValue = $derived<DateValue | undefined>(value ? parseDate(value) : undefined);

	// Custom format: วันศุกร์ ที่ ... เดือน ... ปี พ.ศ. ....
	function formatThaiDateFull(date: Date) {
		const months = [
			'ม.ค.',
			'ก.พ.',
			'มี.ค.',
			'เม.ย.',
			'พ.ค.',
			'มิ.ย.',
			'ก.ค.',
			'ส.ค.',
			'ก.ย.',
			'ต.ค.',
			'พ.ย.',
			'ธ.ค.'
		];
		const day = date.getDate();
		const month = months[date.getMonth()];
		const year = date.getFullYear() + 543;
		return `${day} ${month} ${year}`;
	}

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
				{value && dateValue
					? formatThaiDateFull(dateValue.toDate(getLocalTimeZone()))
					: placeholder}
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-auto p-0">
		<Calendar
			value={dateValue}
			onValueChange={handleValueChange}
			type="single"
			initialFocus
			locale="th-TH"
			captionLayout="dropdown"
		/>
	</Popover.Content>
</Popover.Root>
