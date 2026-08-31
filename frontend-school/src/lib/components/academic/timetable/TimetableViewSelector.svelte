<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { BookOpen, School } from 'lucide-svelte';

	import type { TimetableBoardView } from '$lib/academic/timetable/board-state';

	let {
		value,
		disabled = false,
		onViewChange
	}: {
		value: TimetableBoardView;
		disabled?: boolean;
		onViewChange: (view: TimetableBoardView) => void;
	} = $props();
</script>

<div class="inline-flex rounded-lg border bg-muted/45 p-1" aria-label="มุมมองตารางสอน">
	<Button
		type="button"
		size="sm"
		variant={value === 'homeroom' ? 'default' : 'ghost'}
		class="h-8 rounded-md px-3"
		aria-pressed={value === 'homeroom'}
		{disabled}
		onclick={() => onViewChange('homeroom')}
	>
		<School class="size-3.5" />
		ห้องประจำชั้น
	</Button>
	<Button
		type="button"
		size="sm"
		variant={value === 'learning_group' ? 'default' : 'ghost'}
		class="h-8 rounded-md px-3"
		aria-pressed={value === 'learning_group'}
		{disabled}
		onclick={() => onViewChange('learning_group')}
	>
		<BookOpen class="size-3.5" />
		กลุ่มเรียน
	</Button>
</div>
