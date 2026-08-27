<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import { DateFormatter, getLocalTimeZone, type DateValue } from '@internationalized/date';
	import * as Select from '$lib/components/ui/select';
	import type Calendar from './calendar.svelte';

	type CalendarProps = ComponentProps<typeof Calendar>;

	let {
		captionLayout,
		months,
		monthFormat,
		years,
		yearFormat,
		month,
		locale,
		placeholder = $bindable(),
		monthIndex = 0,
		minValue,
		maxValue,
		disabled = false,
		readonly = false
	}: {
		captionLayout: CalendarProps['captionLayout'];
		months: CalendarProps['months'];
		monthFormat: CalendarProps['monthFormat'];
		years: CalendarProps['years'];
		yearFormat: CalendarProps['yearFormat'];
		month: DateValue;
		placeholder: DateValue | undefined;
		locale: string;
		monthIndex: number;
		minValue?: DateValue;
		maxValue?: DateValue;
		disabled?: boolean;
		readonly?: boolean;
	} = $props();

	let monthOptions = $derived(
		(months?.length ? months : Array.from({ length: 12 }, (_, index) => index + 1)).map(
			(value) => ({ value, label: formatMonth(month.set({ month: value })) })
		)
	);
	let yearOptions = $derived.by(() => {
		const placeholderYear = placeholder?.year ?? month.year;
		const currentYear = new Date().getFullYear();
		const latestYear = Math.max(placeholderYear, currentYear);
		const initialMinYear = latestYear - 100;
		const minYear =
			minValue?.year ?? (placeholderYear < initialMinYear ? placeholderYear - 10 : initialMinYear);
		const maxYear = maxValue?.year ?? latestYear + 10;
		const normalizedMinYear = Math.min(minYear, maxYear);
		const values = years?.length
			? years
			: Array.from(
					{ length: maxYear - normalizedMinYear + 1 },
					(_, index) => normalizedMinYear + index
				);
		return values.map((value) => ({ value, label: formatYear(month.set({ year: value })) }));
	});

	function formatYear(date: DateValue) {
		const dateObj = date.toDate(getLocalTimeZone());
		if (typeof yearFormat === 'function') return yearFormat(dateObj.getFullYear());
		return new DateFormatter(locale, { year: yearFormat }).format(dateObj);
	}

	function formatMonth(date: DateValue) {
		const dateObj = date.toDate(getLocalTimeZone());
		if (typeof monthFormat === 'function') return monthFormat(dateObj.getMonth() + 1);
		return new DateFormatter(locale, { month: monthFormat }).format(dateObj);
	}

	function selectMonth(value: string) {
		if (!placeholder) return;
		placeholder = month.set({ month: Number(value) }).subtract({ months: monthIndex });
	}

	function selectYear(value: string) {
		if (!placeholder) return;
		placeholder = month.set({ year: Number(value) }).subtract({ months: monthIndex });
	}
</script>

{#snippet MonthSelect()}
	<Select.Root type="single" value={String(month.month)} onValueChange={selectMonth}>
		<Select.Trigger
			size="sm"
			aria-label="เลือกเดือน"
			class="h-8 w-[4.75rem] border-input bg-background shadow-xs"
			disabled={!placeholder || disabled || readonly}
		>
			{monthOptions.find((option) => option.value === month.month)?.label ?? formatMonth(month)}
		</Select.Trigger>
		<Select.Content>
			{#each monthOptions as option (option.value)}
				<Select.Item value={String(option.value)}>{option.label}</Select.Item>
			{/each}
		</Select.Content>
	</Select.Root>
{/snippet}

{#snippet YearSelect()}
	<Select.Root type="single" value={String(month.year)} onValueChange={selectYear}>
		<Select.Trigger
			size="sm"
			aria-label="เลือกปี"
			class="h-8 w-26 border-input bg-background shadow-xs"
			disabled={!placeholder || disabled || readonly}
		>
			{yearOptions.find((option) => option.value === month.year)?.label ?? formatYear(month)}
		</Select.Trigger>
		<Select.Content class="max-h-72">
			{#each yearOptions as option (option.value)}
				<Select.Item value={String(option.value)}>{option.label}</Select.Item>
			{/each}
		</Select.Content>
	</Select.Root>
{/snippet}

{#if captionLayout === 'dropdown'}
	{@render MonthSelect()}
	{@render YearSelect()}
{:else if captionLayout === 'dropdown-months'}
	{@render MonthSelect()}
	{formatYear(month)}
{:else if captionLayout === 'dropdown-years'}
	{formatMonth(month)}
	{@render YearSelect()}
{:else}
	{formatMonth(month)} {formatYear(month)}
{/if}
