<script lang="ts">
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		roundLabels,
		visitLabels,
		caseLabels,
		assignmentLabels
	} from '#lib/features/student-care/presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import CareSelect from './CareSelect.svelte';
	let { url, kind }: { url: string; kind: 'rounds' | 'visits' | 'cases' | 'assignments' } =
		$props();
	let status = $state(untrack(() => new URL(url).searchParams.get('status') ?? 'all')),
		due = $state(untrack(() => new URL(url).searchParams.get('followUpBefore') ?? ''));
	const labels = $derived(
		kind === 'rounds'
			? roundLabels
			: kind === 'visits'
				? visitLabels
				: kind === 'cases'
					? caseLabels
					: assignmentLabels
	);
	function apply(event: SubmitEvent) {
		event.preventDefault();
		const next = new URL(url);
		next.searchParams.delete('afterId');
		if (status === 'all') next.searchParams.delete('status');
		else next.searchParams.set('status', status);
		if (kind === 'cases') {
			if (due) next.searchParams.set('followUpBefore', due);
			else next.searchParams.delete('followUpBefore');
		}
		void goto(next.pathname + next.search, { reset: false });
	}
</script>

<form onsubmit={apply} class="flex flex-wrap items-end gap-3 rounded-xl border bg-card p-3 sm:p-4">
	<CareSelect
		id="care-filter-status"
		label="สถานะ"
		value={status}
		options={[
			{ value: 'all', label: 'ทุกสถานะ' },
			...Object.entries(labels).map(([value, label]) => ({ value, label }))
		]}
		onValueChange={(value) => (status = value)}
	/>{#if kind === 'cases'}<div class="space-y-2">
			<Label for="care-filter-due">ติดตามภายในวันที่</Label><DatePicker
				id="care-filter-due"
				ariaLabel="ติดตามภายในวันที่"
				value={due}
				onValueChange={(value) => (due = value ?? '')}
				clearable
			/>
		</div>{/if}<Button type="submit" variant="outline">ใช้ตัวกรอง</Button>
</form>
