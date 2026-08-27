<script lang="ts">
	import type { AcademicYearOption } from '$lib/api/academic-context';
	import * as Select from '$lib/components/ui/select';

	type Props = {
		id: string;
		years: AcademicYearOption[];
		value: string;
		disabled?: boolean;
		onchange: (academicYearId: string) => void;
	};

	let { id, years, value, disabled = false, onchange }: Props = $props();
</script>

<Select.Root type="single" {value} {disabled} onValueChange={onchange}>
	<Select.Trigger {id} class="w-full">
		{years.find((year) => year.id === value)?.name ?? 'เลือกปีการศึกษา'}
	</Select.Trigger>
	<Select.Content>
		{#each years as year (year.id)}
			<Select.Item value={year.id}>{year.name}</Select.Item>
		{/each}
	</Select.Content>
</Select.Root>
