<script lang="ts">
	import { type DateValue, getLocalTimeZone, parseDate } from '@internationalized/date';
	import CalendarIcon from 'lucide-svelte/icons/calendar';
	import XIcon from 'lucide-svelte/icons/x';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Calendar } from '$lib/components/ui/calendar/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import { cn } from '$lib/utils.js';

	interface Props {
		id?: string;
		value?: string;
		placeholder?: string;
		class?: string;
		disabled?: boolean;
		required?: boolean;
		clearable?: boolean;
		ariaLabel?: string;
		onValueChange?: (value: string | undefined) => void;
	}

	let {
		id,
		value = $bindable(),
		placeholder = 'เลือกวันที่',
		class: className = '',
		disabled = false,
		required = false,
		clearable = false,
		ariaLabel = 'เลือกวันที่',
		onValueChange
	}: Props = $props();

	let open = $state(false);
	let dateValue = $derived<DateValue | undefined>(safeParseDate(value));

	function safeParseDate(rawValue: string | undefined): DateValue | undefined {
		if (!rawValue || !/^\d{4}-\d{2}-\d{2}$/.test(rawValue)) return undefined;
		try {
			return parseDate(rawValue);
		} catch {
			return undefined;
		}
	}

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

	function handleValueChange(newValue: DateValue | undefined) {
		const isoString = newValue?.toString();
		value = isoString;
		onValueChange?.(isoString);
		if (newValue) open = false;
	}

	function clearValue() {
		value = undefined;
		onValueChange?.(undefined);
	}
</script>

<div class="flex w-full items-center gap-1.5">
	<Popover.Root bind:open>
		<Popover.Trigger>
			{#snippet child({ props })}
				<Button
					type="button"
					variant="outline"
					class={cn(
						'min-w-0 flex-1 justify-start text-start font-normal',
						!dateValue && 'text-muted-foreground',
						className
					)}
					{...props}
					{id}
					{disabled}
					aria-label={ariaLabel}
					aria-required={required}
					data-required={required || undefined}
				>
					<CalendarIcon class="me-2 size-4 shrink-0" />
					<span class="truncate">
						{dateValue ? formatThaiDateFull(dateValue.toDate(getLocalTimeZone())) : placeholder}
					</span>
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
				{disabled}
			/>
		</Popover.Content>
	</Popover.Root>
	{#if clearable && value && !disabled}
		<Button
			type="button"
			variant="ghost"
			size="icon"
			class="shrink-0"
			aria-label="ล้างวันที่"
			onclick={clearValue}
		>
			<XIcon class="size-4" />
		</Button>
	{/if}
</div>
