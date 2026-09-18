<script lang="ts">
	import type { TimetableBlockWorkspaceRoom } from '$lib/api/timetable';
	import * as Select from '$lib/components/ui/select';
	import { Building2 } from 'lucide-svelte';

	const noRoomValue = '__none__';

	let {
		rooms,
		value,
		disabled = false,
		onValueChange
	}: {
		rooms: TimetableBlockWorkspaceRoom[];
		value: string | null;
		disabled?: boolean;
		onValueChange: (roomId: string | null) => void;
	} = $props();

	const selectedRoom = $derived(rooms.find((room) => room.id === value) ?? null);
	const availableRooms = $derived(rooms.filter((room) => room.status.toUpperCase() === 'ACTIVE'));

	function roomLabel(room: TimetableBlockWorkspaceRoom): string {
		return room.code ? `${room.code} · ${room.name}` : room.name;
	}
</script>

<Select.Root
	type="single"
	value={value ?? noRoomValue}
	{disabled}
	onValueChange={(next) => onValueChange(next === noRoomValue ? null : next)}
>
	<Select.Trigger class="w-full min-w-0" aria-label="เลือกห้องเรียน">
		<span class="flex min-w-0 items-center gap-1.5">
			<Building2 class="size-3.5 shrink-0" />
			<span class="truncate">{selectedRoom ? roomLabel(selectedRoom) : 'ไม่ระบุห้อง'}</span>
		</span>
	</Select.Trigger>
	<Select.Content>
		<Select.Item value={noRoomValue}>ไม่ระบุห้อง</Select.Item>
		{#each availableRooms as room (room.id)}
			<Select.Item value={room.id}>{roomLabel(room)}</Select.Item>
		{/each}
	</Select.Content>
</Select.Root>
