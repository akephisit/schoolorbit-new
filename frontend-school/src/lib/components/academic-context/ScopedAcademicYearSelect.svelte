<script lang="ts">
	import type { AcademicYearOption } from '$lib/api/academic-context';

	type Props = {
		id: string;
		years: AcademicYearOption[];
		value: string;
		disabled?: boolean;
		onchange: (academicYearId: string) => void;
	};

	let { id, years, value, disabled = false, onchange }: Props = $props();
</script>

<select
	{id}
	class="border-input bg-background h-9 w-full rounded-md border px-3 text-sm"
	{value}
	{disabled}
	onchange={(event) => onchange(event.currentTarget.value)}
>
	{#each years as year (year.id)}
		<option value={year.id}>{year.name}</option>
	{/each}
</select>
