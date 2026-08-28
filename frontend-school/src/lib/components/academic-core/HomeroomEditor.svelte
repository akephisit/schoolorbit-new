<script lang="ts">
	import type {
		CreateHomeroomRequest,
		GradeLevelOption,
		Homeroom,
		HomeroomAdvisor,
		ReplaceHomeroomAdvisorsRequest,
		StaffOption,
		StudyProgramOption,
		UpdateHomeroomRequest
	} from '$lib/api/academic-core';
	import { customNameFromStored } from '$lib/academic-core/foundation-presentation';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { DoorOpen, Pencil, Plus, Trash2, UserRoundCog, Users } from 'lucide-svelte';

	type HomeroomDraft = Omit<CreateHomeroomRequest, 'academicYearId'>;
	type AdvisorDraft = ReplaceHomeroomAdvisorsRequest['advisors'][number];

	let {
		homerooms,
		gradeLevelOptions,
		programOptions,
		advisorsByHomeroom,
		canManage = false,
		onCreate,
		onUpdate,
		onLoadStaffOptions,
		onSaveAdvisors
	}: {
		homerooms: Homeroom[];
		gradeLevelOptions: GradeLevelOption[];
		programOptions: StudyProgramOption[];
		advisorsByHomeroom: Map<string, HomeroomAdvisor[]>;
		canManage?: boolean;
		onCreate: (draft: HomeroomDraft) => Promise<Homeroom>;
		onUpdate: (room: Homeroom, draft: UpdateHomeroomRequest) => Promise<Homeroom>;
		onLoadStaffOptions: () => Promise<StaffOption[]>;
		onSaveAdvisors: (
			room: Homeroom,
			advisors: ReplaceHomeroomAdvisorsRequest['advisors']
		) => Promise<HomeroomAdvisor[]>;
	} = $props();

	let roomDialogOpen = $state(false);
	let editingRoom = $state<Homeroom | null>(null);
	let roomDraft = $state<HomeroomDraft>(emptyRoomDraft());
	let roomBusy = $state(false);
	let roomError = $state('');

	let advisorDialogOpen = $state(false);
	let advisorRoom = $state<Homeroom | null>(null);
	let advisorDrafts = $state<AdvisorDraft[]>([]);
	let staffOptions = $state<StaffOption[]>([]);
	let loadingStaff = $state(false);
	let advisorBusy = $state(false);
	let advisorError = $state('');

	const selectedGrade = $derived(
		gradeLevelOptions.find((option) => option.id === roomDraft.gradeLevelId) ?? null
	);
	const standardRoomName = $derived(
		selectedGrade?.short_name && roomDraft.roomNumber.trim()
			? `${selectedGrade.short_name}/${roomDraft.roomNumber.trim()}`
			: 'ระบบจะสร้างชื่อจากระดับชั้นและเลขห้อง'
	);
	const roomPreview = $derived(roomDraft.customName?.trim() || standardRoomName);

	function emptyRoomDraft(): HomeroomDraft {
		return {
			customName: null,
			gradeLevelId: '',
			roomNumber: '',
			studyProgramId: '',
			capacity: 40
		};
	}

	function gradeLabel(id: string): string {
		return gradeLevelOptions.find((option) => option.id === id)?.name ?? 'ไม่พบระดับชั้นที่อ้างอิง';
	}

	function programLabel(id: string): string {
		const program = programOptions.find((option) => option.id === id);
		return program ? `${program.curriculumName} · ${program.name}` : 'ไม่พบแผนการเรียนที่อ้างอิง';
	}

	function customNameForRoom(room: Homeroom): string | null {
		const grade = gradeLevelOptions.find((option) => option.id === room.gradeLevelId);
		const standard =
			grade?.short_name && room.roomNumber ? `${grade.short_name}/${room.roomNumber}` : '';
		if (!standard) return room.name;
		return customNameFromStored(room.name, standard) || null;
	}

	function openCreateDialog() {
		editingRoom = null;
		roomDraft = emptyRoomDraft();
		roomError = '';
		roomDialogOpen = true;
	}

	function openEditDialog(room: Homeroom) {
		editingRoom = room;
		roomDraft = {
			customName: customNameForRoom(room),
			gradeLevelId: room.gradeLevelId,
			roomNumber: room.roomNumber ?? '',
			studyProgramId: room.studyProgramId,
			capacity: room.capacity
		};
		roomError = '';
		roomDialogOpen = true;
	}

	async function saveRoom(event: SubmitEvent) {
		event.preventDefault();
		roomError = '';
		if (!roomDraft.gradeLevelId || !roomDraft.studyProgramId || !roomDraft.roomNumber.trim()) {
			roomError = 'กรุณาเลือกระดับชั้น แผนการเรียน และระบุเลขห้อง';
			return;
		}
		const common = {
			...roomDraft,
			roomNumber: roomDraft.roomNumber.trim(),
			customName: roomDraft.customName?.trim() || null
		};
		roomBusy = true;
		try {
			if (editingRoom) {
				await onUpdate(editingRoom, { ...common, rowVersion: editingRoom.rowVersion });
			} else {
				await onCreate(common);
			}
			roomDialogOpen = false;
		} catch (error) {
			roomError = error instanceof Error ? error.message : 'บันทึกห้องประจำชั้นไม่สำเร็จ';
		} finally {
			roomBusy = false;
		}
	}

	async function openAdvisorDialog(room: Homeroom) {
		advisorRoom = room;
		advisorDrafts = (advisorsByHomeroom.get(room.id) ?? []).map((advisor) => ({
			userId: advisor.userId,
			role: advisor.role
		}));
		advisorError = '';
		advisorDialogOpen = true;
		if (staffOptions.length > 0) return;
		loadingStaff = true;
		try {
			staffOptions = await onLoadStaffOptions();
		} catch (error) {
			advisorError = error instanceof Error ? error.message : 'โหลดรายชื่อครูไม่สำเร็จ';
		} finally {
			loadingStaff = false;
		}
	}

	function addAdvisorRow() {
		advisorDrafts = [...advisorDrafts, { userId: '', role: 'primary' }];
	}

	function removeAdvisorRow(index: number) {
		advisorDrafts = advisorDrafts.filter((_, itemIndex) => itemIndex !== index);
	}

	async function saveAdvisors(event: SubmitEvent) {
		event.preventDefault();
		if (!advisorRoom) return;
		advisorError = '';
		if (advisorDrafts.some((advisor) => !advisor.userId)) {
			advisorError = 'กรุณาเลือกครูให้ครบทุกรายการ';
			return;
		}
		if (new Set(advisorDrafts.map((advisor) => advisor.userId)).size !== advisorDrafts.length) {
			advisorError = 'ครูหนึ่งคนไม่ควรถูกเพิ่มซ้ำในห้องเดียวกัน';
			return;
		}
		advisorBusy = true;
		try {
			await onSaveAdvisors(advisorRoom, advisorDrafts);
			advisorDialogOpen = false;
		} catch (error) {
			advisorError = error instanceof Error ? error.message : 'บันทึกครูที่ปรึกษาไม่สำเร็จ';
		} finally {
			advisorBusy = false;
		}
	}
</script>

<div class="space-y-4">
	<div
		class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card p-3 sm:p-4"
	>
		<div>
			<p class="text-sm font-semibold">{homerooms.length} ห้องประจำชั้น</p>
			<p class="text-xs text-muted-foreground">
				ชื่อห้องและรหัสสร้างจากระดับชั้นกับเลขห้องโดยอัตโนมัติ
			</p>
		</div>
		{#if canManage}
			<Button type="button" onclick={openCreateDialog}
				><Plus class="size-4" /> เพิ่มห้องประจำชั้น</Button
			>
		{/if}
	</div>

	<div class="overflow-x-auto rounded-xl border bg-card">
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head class="min-w-40 ps-5">ห้อง</Table.Head>
					<Table.Head class="min-w-44">ระดับชั้น</Table.Head>
					<Table.Head class="min-w-56">แผนการเรียน</Table.Head>
					<Table.Head class="text-center">ความจุ</Table.Head>
					<Table.Head class="text-center">ครูที่ปรึกษา</Table.Head>
					<Table.Head class="w-28"><span class="sr-only">จัดการ</span></Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each homerooms as room (room.id)}
					<Table.Row>
						<Table.Cell class="border-s-4 border-s-primary ps-5">
							<div class="flex items-center gap-3">
								<div class="rounded-lg bg-primary/10 p-2 text-primary">
									<DoorOpen class="size-4" />
								</div>
								<div>
									<p class="font-medium">{room.name}</p>
									<p class="text-xs text-muted-foreground">เลขห้อง {room.roomNumber}</p>
								</div>
							</div>
						</Table.Cell>
						<Table.Cell>{gradeLabel(room.gradeLevelId)}</Table.Cell>
						<Table.Cell class="whitespace-normal">{programLabel(room.studyProgramId)}</Table.Cell>
						<Table.Cell class="text-center tabular-nums">{room.capacity}</Table.Cell>
						<Table.Cell class="text-center">
							<Badge variant="secondary"
								><Users class="size-3" /> {advisorsByHomeroom.get(room.id)?.length ?? 0} คน</Badge
							>
						</Table.Cell>
						<Table.Cell>
							{#if canManage}
								<div class="flex justify-end gap-1">
									<Button
										type="button"
										size="icon-sm"
										variant="ghost"
										onclick={() => void openAdvisorDialog(room)}
										aria-label={`จัดครูที่ปรึกษา ${room.name}`}
									>
										<UserRoundCog class="size-4" />
									</Button>
									<Button
										type="button"
										size="icon-sm"
										variant="ghost"
										onclick={() => openEditDialog(room)}
										aria-label={`แก้ไข ${room.name}`}
									>
										<Pencil class="size-4" />
									</Button>
								</div>
							{/if}
						</Table.Cell>
					</Table.Row>
				{:else}
					<Table.Row
						><Table.Cell colspan={6} class="h-32 text-center text-muted-foreground"
							>ปีที่เลือกยังไม่มีห้องประจำชั้น</Table.Cell
						></Table.Row
					>
				{/each}
			</Table.Body>
		</Table.Root>
	</div>
</div>

<Dialog.Root bind:open={roomDialogOpen}>
	<Dialog.Content class="sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>{editingRoom ? `แก้ไข ${editingRoom.name}` : 'เพิ่มห้องประจำชั้น'}</Dialog.Title
			>
			<Dialog.Description
				>เลือกระดับชั้น เลขห้อง และแผนการเรียน ระบบจะสร้างชื่อกับรหัสให้ตรงกัน</Dialog.Description
			>
		</Dialog.Header>
		<form class="space-y-4" onsubmit={saveRoom}>
			<div class="rounded-xl border border-primary/20 bg-primary/[0.04] p-3">
				<p class="text-xs font-medium text-primary">ชื่อห้องที่จะแสดง</p>
				<p class="mt-1 font-semibold">{roomPreview}</p>
			</div>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-1.5">
					<Label for="homeroom-grade">ระดับชั้น</Label>
					<Select.Root type="single" bind:value={roomDraft.gradeLevelId}>
						<Select.Trigger id="homeroom-grade" class="w-full"
							>{selectedGrade?.name ?? 'เลือกระดับชั้น'}</Select.Trigger
						>
						<Select.Content
							>{#each gradeLevelOptions as option (option.id)}<Select.Item value={option.id}
									>{option.name}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
				</div>
				<div class="space-y-1.5">
					<Label for="homeroom-number">เลขห้อง</Label>
					<Input id="homeroom-number" bind:value={roomDraft.roomNumber} placeholder="3" required />
				</div>
			</div>
			<div class="space-y-1.5">
				<Label for="homeroom-program">แผนการเรียน</Label>
				<Select.Root type="single" bind:value={roomDraft.studyProgramId}>
					<Select.Trigger id="homeroom-program" class="w-full"
						>{programLabel(roomDraft.studyProgramId).replace(
							'ไม่พบแผนการเรียนที่อ้างอิง',
							'เลือกแผนการเรียน'
						)}</Select.Trigger
					>
					<Select.Content
						>{#each programOptions as option (option.id)}<Select.Item value={option.id}
								>{option.curriculumName} · {option.name}</Select.Item
							>{/each}</Select.Content
					>
				</Select.Root>
			</div>
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-1.5">
					<Label for="homeroom-capacity">ความจุ</Label>
					<Input
						id="homeroom-capacity"
						type="number"
						min="1"
						bind:value={roomDraft.capacity}
						required
					/>
				</div>
				<div class="space-y-1.5">
					<Label for="homeroom-custom-name">ชื่ออื่น (ไม่บังคับ)</Label>
					<Input
						id="homeroom-custom-name"
						bind:value={roomDraft.customName}
						placeholder="เว้นว่างเพื่อใช้ชื่อมาตรฐาน"
					/>
				</div>
			</div>
			{#if roomError}<p role="alert" class="text-sm text-destructive">{roomError}</p>{/if}
			<Dialog.Footer
				><Button type="submit" disabled={roomBusy}
					>{editingRoom ? 'บันทึกห้อง' : 'สร้างห้อง'}</Button
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={advisorDialogOpen}>
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>ครูที่ปรึกษา {advisorRoom?.name}</Dialog.Title>
			<Dialog.Description
				>กำหนดครูที่ปรึกษาหลักหรือครูที่ปรึกษาร่วม โดยเลือกจากบัญชีบุคลากร</Dialog.Description
			>
		</Dialog.Header>
		<form class="space-y-4" onsubmit={saveAdvisors}>
			{#if loadingStaff}
				<p class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
					กำลังโหลดรายชื่อครู…
				</p>
			{:else}
				<div class="space-y-3">
					{#each advisorDrafts as advisor, index (`advisor-${index}`)}
						<div class="grid gap-2 rounded-xl border p-3 sm:grid-cols-[minmax(0,1fr)_180px_36px]">
							<Select.Root type="single" bind:value={advisor.userId}>
								<Select.Trigger class="w-full"
									>{staffOptions.find((staff) => staff.id === advisor.userId)?.name ??
										(advisor.userId ? 'ไม่พบข้อมูลครูที่อ้างอิง' : 'เลือกครู')}</Select.Trigger
								>
								<Select.Content
									>{#each staffOptions as staff (staff.id)}<Select.Item value={staff.id}
											>{staff.title ? `${staff.title} ` : ''}{staff.name}</Select.Item
										>{/each}</Select.Content
								>
							</Select.Root>
							<Select.Root type="single" bind:value={advisor.role}>
								<Select.Trigger class="w-full"
									>{advisor.role === 'primary'
										? 'ครูที่ปรึกษาหลัก'
										: 'ครูที่ปรึกษาร่วม'}</Select.Trigger
								>
								<Select.Content
									><Select.Item value="primary">ครูที่ปรึกษาหลัก</Select.Item><Select.Item
										value="secondary">ครูที่ปรึกษาร่วม</Select.Item
									></Select.Content
								>
							</Select.Root>
							<Button
								type="button"
								size="icon-sm"
								variant="ghost"
								onclick={() => removeAdvisorRow(index)}
								aria-label={`ลบครูที่ปรึกษารายการ ${index + 1}`}><Trash2 class="size-4" /></Button
							>
						</div>
					{:else}
						<p class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
							ยังไม่มีครูที่ปรึกษา
						</p>
					{/each}
				</div>
				<Button type="button" variant="outline" onclick={addAdvisorRow}
					><Plus class="size-4" /> เพิ่มครูที่ปรึกษา</Button
				>
			{/if}
			{#if advisorError}<p role="alert" class="text-sm text-destructive">{advisorError}</p>{/if}
			<Dialog.Footer
				><Button type="submit" disabled={loadingStaff || advisorBusy}>บันทึกครูที่ปรึกษา</Button
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>
