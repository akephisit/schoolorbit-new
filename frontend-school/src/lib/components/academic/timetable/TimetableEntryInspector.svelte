<script lang="ts">
	import type { TimetableEntry, TimetableWorkspace } from '$lib/api/timetable';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';

	import TimetableInstructorPicker from './TimetableInstructorPicker.svelte';

	type InstructorOption = {
		id: string;
		displayName: string;
		role: 'primary' | 'secondary' | 'assistant';
	};

	let {
		open = $bindable(false),
		entry,
		rooms,
		instructorOptions,
		readOnly = false,
		busy = false,
		onSave,
		onMove,
		onRemove
	}: {
		open?: boolean;
		entry: TimetableEntry | null;
		rooms: TimetableWorkspace['rooms'];
		instructorOptions: InstructorOption[];
		readOnly?: boolean;
		busy?: boolean;
		onSave: (value: { roomId: string | null; instructorIds: string[] }) => void;
		onMove?: (entry: TimetableEntry) => void;
		onRemove?: (entry: TimetableEntry) => void;
	} = $props();

	const homeroomRoomValue = '__homeroom_room__';
	let selectedRoomValue = $derived(entry?.roomId ?? homeroomRoomValue);
	let selectedInstructorIds = $derived(entry?.instructors.map((teacher) => teacher.userId) ?? []);
	const selectedRoom = $derived(rooms.find((room) => room.id === selectedRoomValue));
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>{readOnly ? 'รายละเอียดคาบ' : 'แก้รายละเอียดคาบ'}</Dialog.Title>
			<Dialog.Description>
				{entry?.offeringCode ?? entry?.entryType ?? ''} · {entry?.offeringName ??
					entry?.title ??
					''}
			</Dialog.Description>
		</Dialog.Header>
		{#if entry}
			<div class="space-y-5 py-2">
				<TimetableInstructorPicker
					options={instructorOptions}
					bind:value={selectedInstructorIds}
					disabled={readOnly || busy}
				/>
				<div class="space-y-2">
					<Label>ห้องเรียน</Label>
					<Select.Root type="single" bind:value={selectedRoomValue} disabled={readOnly || busy}>
						<Select.Trigger class="w-full"
							>{selectedRoom?.code ?? selectedRoom?.name ?? 'ใช้ห้องประจำชั้น'}</Select.Trigger
						>
						<Select.Content>
							<Select.Item value={homeroomRoomValue}>ใช้ห้องประจำชั้น</Select.Item>
							{#each rooms as room (room.id)}
								<Select.Item value={room.id}>{room.code ?? room.name} · {room.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>
		{/if}
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)}>ปิด</Button>
			{#if entry && !readOnly}
				<Button variant="outline" disabled={busy} onclick={() => onMove?.(entry)}>ย้ายคาบ</Button>
				<Button variant="destructive" disabled={busy} onclick={() => onRemove?.(entry)}
					>นำออกจากตาราง</Button
				>
				<Button
					disabled={busy || (instructorOptions.length > 0 && selectedInstructorIds.length === 0)}
					onclick={() =>
						onSave({
							roomId: selectedRoomValue === homeroomRoomValue ? null : selectedRoomValue,
							instructorIds: selectedInstructorIds
						})}
				>
					{busy ? 'กำลังบันทึก...' : 'บันทึกการเปลี่ยนแปลง'}
				</Button>
			{/if}
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
