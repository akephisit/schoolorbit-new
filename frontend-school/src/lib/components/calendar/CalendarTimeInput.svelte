<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';
	import { normalizeCalendarTime } from '#lib/utils/calendar.js';
	let {
		id,
		value = $bindable(''),
		disabled = false,
		required = true
	}: { id: string; value?: string; disabled?: boolean; required?: boolean } = $props();
	let input = $state<HTMLInputElement | null>(null);
	let touched = $state(false);
	const invalid = $derived(!!value && !normalizeCalendarTime(value));
	const hintId = $derived(`${id}-hint`);
	function validate(element: HTMLInputElement, text: string) {
		element.setCustomValidity(
			text && !normalizeCalendarTime(text) ? 'กรอกเวลา 24 ชั่วโมง เช่น 08:30 หรือ 0830' : ''
		);
	}
	$effect(() => {
		if (input) validate(input, value);
	});
</script>

<Input
	{id}
	bind:ref={input}
	bind:value
	type="text"
	inputmode="numeric"
	placeholder="08:30"
	maxlength={5}
	{required}
	{disabled}
	aria-describedby={hintId}
	aria-invalid={touched && invalid}
	oninput={(event) => validate(event.currentTarget, event.currentTarget.value)}
	onblur={() => {
		touched = true;
		value = normalizeCalendarTime(value) ?? value;
	}}
/>
<p
	id={hintId}
	class={touched && invalid ? 'text-xs text-destructive' : 'text-xs text-muted-foreground'}
>
	{touched && invalid ? 'กรอกเวลา 00:00–23:59' : '24 ชั่วโมง เช่น 08:30 หรือ 0830'}
</p>
