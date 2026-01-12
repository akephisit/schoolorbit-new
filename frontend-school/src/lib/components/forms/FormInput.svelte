<script lang="ts">
	import type { ValidationError } from '$lib/validation';

	interface Props {
		label: string;
		name: string;
		type?: string;
		value?: string | number;
		placeholder?: string;
		required?: boolean;
		disabled?: boolean;
		errors?: ValidationError[];
		class?: string;
		onchange?: (value: string) => void;
	}

	let {
		label,
		name,
		type = 'text',
		value = $bindable(''),
		placeholder = '',
		required = false,
		disabled = false,
		errors = [],
		class: className = '',
		onchange
	}: Props = $props();

	// Get error for this field
	const fieldError = $derived(errors?.find((err) => err.field === name)?.message);
	const hasError = $derived(!!fieldError);

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement;
		value = target.value;
		if (onchange) {
			onchange(target.value);
		}
	}
</script>

<div class="form-field {className}">
	<label for={name} class="form-label">
		{label}
		{#if required}
			<span class="text-red-500">*</span>
		{/if}
	</label>

	<input
		id={name}
		{name}
		{type}
		{placeholder}
		{required}
		{disabled}
		{value}
		oninput={handleInput}
		class="form-input"
		class:error={hasError}
		aria-invalid={hasError}
		aria-describedby={hasError ? `${name}-error` : undefined}
	/>

	{#if hasError}
		<p id="{name}-error" class="form-error">
			{fieldError}
		</p>
	{/if}
</div>

<style>
	.form-field {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}

	.form-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: hsl(var(--foreground));
	}

	.form-input {
		padding: 0.5rem 0.75rem;
		border: 1px solid hsl(var(--border));
		border-radius: 0.375rem;
		font-size: 0.875rem;
		transition: all 0.15s ease;
	}

	.form-input:focus {
		outline: none;
		border-color: hsl(var(--primary));
		box-shadow: 0 0 0 3px hsl(var(--primary) / 0.1);
	}

	.form-input:disabled {
		opacity: 0.5;
		cursor: not-allowed;
		background-color: hsl(var(--muted));
	}

	.form-input.error {
		border-color: hsl(var(--destructive));
	}

	.form-input.error:focus {
		border-color: hsl(var(--destructive));
		box-shadow: 0 0 0 3px hsl(var(--destructive) / 0.1);
	}

	.form-error {
		font-size: 0.75rem;
		color: hsl(var(--destructive));
		margin-top: -0.25rem;
	}
</style>
