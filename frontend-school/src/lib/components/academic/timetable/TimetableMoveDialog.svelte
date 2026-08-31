<script lang="ts">
	import type { TimetableEntry, TimetableWorkspace } from '$lib/api/timetable';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { Label } from '$lib/components/ui/label';

	let {
		open = $bindable(false),
		entry,
		periods,
		busy = false,
		onConfirm
	}: {
		open?: boolean;
		entry: TimetableEntry | null;
		periods: TimetableWorkspace['bellPeriods'];
		busy?: boolean;
		onConfirm: (dayOfWeek: string, periodId: string) => void;
	} = $props();

	const days = [
		{ id: 'MON', label: 'วันจันทร์' },
		{ id: 'TUE', label: 'วันอังคาร' },
		{ id: 'WED', label: 'วันพุธ' },
		{ id: 'THU', label: 'วันพฤหัสบดี' },
		{ id: 'FRI', label: 'วันศุกร์' }
	];
	let selectedDay = $derived(entry?.dayOfWeek ?? days[0].id);
	let selectedPeriodId = $derived(entry?.bellSchedulePeriodId ?? periods[0]?.id ?? '');
	const selectedDayLabel = $derived(
		days.find((day) => day.id === selectedDay)?.label ?? 'เลือกวัน'
	);
	const selectedPeriod = $derived(periods.find((period) => period.id === selectedPeriodId));
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>ย้ายคาบ</Dialog.Title>
			<Dialog.Description>
				{entry?.offeringCode ?? entry?.entryType ?? ''} · {entry?.offeringName ??
					entry?.title ??
					'เลือกรายการ'}
			</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-4 py-2">
			<div class="space-y-2">
				<Label>วัน</Label>
				<Select.Root type="single" bind:value={selectedDay} disabled={busy}>
					<Select.Trigger class="w-full">{selectedDayLabel}</Select.Trigger>
					<Select.Content>
						{#each days as day (day.id)}<Select.Item value={day.id}>{day.label}</Select.Item>{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="space-y-2">
				<Label>คาบ</Label>
				<Select.Root type="single" bind:value={selectedPeriodId} disabled={busy}>
					<Select.Trigger class="w-full">{selectedPeriod?.name ?? 'เลือกคาบ'}</Select.Trigger>
					<Select.Content>
						{#each periods as period (period.id)}
							<Select.Item value={period.id}
								>{period.name ?? `คาบที่ ${period.orderIndex}`} · {period.startTime.slice(
									0,
									5
								)}–{period.endTime.slice(0, 5)}</Select.Item
							>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
		</div>
		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => (open = false)}>ยกเลิก</Button>
			<Button
				disabled={busy || !entry || !selectedPeriodId}
				onclick={() => onConfirm(selectedDay, selectedPeriodId)}
			>
				{busy ? 'กำลังตรวจสอบ...' : 'ย้ายคาบ'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
