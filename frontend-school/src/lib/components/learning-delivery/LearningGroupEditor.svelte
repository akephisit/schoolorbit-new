<script lang="ts">
	import type {
		DeliveryManagementOptions,
		LearningGroup,
		ReplaceLearningGroupHomeroomsRequest,
		ReplaceLearningGroupTeachersRequest,
		TeacherAssignment,
		UpdateLearningGroupRequest
	} from '$lib/api/learning-delivery';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import { Plus, X } from 'lucide-svelte';
	import { untrack } from 'svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';

	let {
		group,
		managementOptions,
		canManage = false,
		onSaveGroup,
		onReplaceTeachers,
		onReplaceHomerooms
	}: {
		group: LearningGroup;
		managementOptions: DeliveryManagementOptions;
		canManage?: boolean;
		onSaveGroup: (request: UpdateLearningGroupRequest) => Promise<void>;
		onReplaceTeachers: (request: ReplaceLearningGroupTeachersRequest) => Promise<void>;
		onReplaceHomerooms: (request: ReplaceLearningGroupHomeroomsRequest) => Promise<void>;
	} = $props();

	let details = $state(
		untrack(() => ({
			code: group.code,
			name: group.name,
			description: group.description ?? '',
			capacity: group.capacity ?? null,
			preferredRoomIds: [...group.preferredRoomIds]
		}))
	);
	let teachers = $state<TeacherAssignment[]>(
		untrack(() => group.teacherAssignments.map((item) => ({ ...item })))
	);
	let homeroomIds = $state<string[]>(untrack(() => [...group.homeroomIds]));
	let selectedRoomId = $state('');
	let selectedTeacherId = $state('');
	let selectedTeacherRole = $state<TeacherAssignment['role']>('primary');
	let selectedHomeroomId = $state('');
	let busySection = $state<'details' | 'teachers' | 'homerooms' | null>(null);
	let errorMessage = $state('');

	const roomOptions = $derived(
		managementOptions.rooms.map((room) => ({
			id: room.id,
			label: room.name_th,
			description: [room.code, room.building_name].filter(Boolean).join(' · ')
		}))
	);
	const teacherOptions = $derived(
		managementOptions.teachers.map((teacher) => ({
			id: teacher.id,
			label: [teacher.title, teacher.name].filter(Boolean).join(' ')
		}))
	);
	const homeroomOptions = $derived(
		managementOptions.homerooms.map((homeroom) => ({
			id: homeroom.id,
			label: homeroom.name,
			description: homeroom.gradeLevel
		}))
	);

	function roomName(id: string) {
		return managementOptions.rooms.find((room) => room.id === id)?.name_th ?? 'ห้องที่เลิกใช้งาน';
	}

	function teacherName(id: string) {
		const teacher = managementOptions.teachers.find((item) => item.id === id);
		return teacher ? [teacher.title, teacher.name].filter(Boolean).join(' ') : 'ครูที่เลิกใช้งาน';
	}

	function homeroomName(id: string) {
		return (
			managementOptions.homerooms.find((homeroom) => homeroom.id === id)?.name ??
			'ห้องที่เลิกใช้งาน'
		);
	}

	function roleName(role: TeacherAssignment['role']) {
		if (role === 'primary') return 'ผู้สอนหลัก';
		if (role === 'secondary') return 'ผู้สอนร่วม';
		return 'ผู้ช่วยสอน';
	}

	function addRoom() {
		if (!selectedRoomId || details.preferredRoomIds.includes(selectedRoomId)) return;
		details.preferredRoomIds = [...details.preferredRoomIds, selectedRoomId];
		selectedRoomId = '';
	}

	function addTeacher() {
		if (!selectedTeacherId) return;
		const assignment = { teacherId: selectedTeacherId, role: selectedTeacherRole };
		const existing = teachers.findIndex((item) => item.teacherId === selectedTeacherId);
		teachers =
			existing === -1
				? [...teachers, assignment]
				: teachers.map((item, index) => (index === existing ? assignment : item));
		selectedTeacherId = '';
	}

	function addHomeroom() {
		if (!selectedHomeroomId || homeroomIds.includes(selectedHomeroomId)) return;
		homeroomIds = [...homeroomIds, selectedHomeroomId];
		selectedHomeroomId = '';
	}

	async function perform(
		section: 'details' | 'teachers' | 'homerooms',
		action: () => Promise<void>,
		fallback: string
	) {
		busySection = section;
		errorMessage = '';
		try {
			await action();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : fallback;
		} finally {
			busySection = null;
		}
	}

	async function saveDetails(event: SubmitEvent) {
		event.preventDefault();
		await perform(
			'details',
			() =>
				onSaveGroup({
					code: details.code.trim(),
					name: details.name.trim(),
					description: details.description.trim() || null,
					capacity: details.capacity,
					preferredRoomIds: details.preferredRoomIds,
					rowVersion: group.rowVersion
				}),
			'บันทึกรายละเอียดกลุ่มไม่สำเร็จ'
		);
	}
</script>

<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
	<header class="border-b bg-muted/25 p-4">
		<h2 class="font-semibold">จัดการกลุ่ม · {group.name}</h2>
		<p class="mt-1 text-sm text-muted-foreground">
			แต่ละส่วนบันทึกแยกกัน เพื่อให้แก้ครู ห้องต้นทาง หรือห้องเรียนได้โดยไม่ทับข้อมูลอื่น
		</p>
	</header>

	<div class="grid gap-6 p-4 xl:grid-cols-3">
		<form class="space-y-4" onsubmit={saveDetails}>
			<div>
				<h3 class="font-medium">ข้อมูลกลุ่มและห้องเรียน</h3>
				<p class="text-xs text-muted-foreground">ชื่อที่ครูเห็นและห้องที่เหมาะกับการจัดตาราง</p>
			</div>
			<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-1 2xl:grid-cols-2">
				<div class="space-y-2">
					<Label for="group-editor-code">รหัสกลุ่ม</Label>
					<Input id="group-editor-code" bind:value={details.code} required disabled={!canManage} />
				</div>
				<div class="space-y-2">
					<Label for="group-editor-capacity">ความจุ (ถ้ามี)</Label>
					<Input
						id="group-editor-capacity"
						type="number"
						min="1"
						bind:value={details.capacity}
						disabled={!canManage}
					/>
				</div>
			</div>
			<div class="space-y-2">
				<Label for="group-editor-name">ชื่อกลุ่ม</Label>
				<Input id="group-editor-name" bind:value={details.name} required disabled={!canManage} />
			</div>
			<div class="space-y-2">
				<Label for="group-editor-description">คำอธิบาย (ถ้ามี)</Label>
				<Textarea
					id="group-editor-description"
					bind:value={details.description}
					rows={2}
					disabled={!canManage}
				/>
			</div>
			<div class="space-y-2">
				<Label>ห้องเรียนที่ต้องการ</Label>
				<div class="flex gap-2">
					<div class="min-w-0 flex-1">
						<DeliveryOptionCombobox
							bind:value={selectedRoomId}
							options={roomOptions.filter((room) => !details.preferredRoomIds.includes(room.id))}
							placeholder="เลือกห้องเรียน"
							searchPlaceholder="ค้นหาชื่อหรือรหัสห้อง..."
							disabled={!canManage}
						/>
					</div>
					<Button
						type="button"
						size="icon"
						variant="outline"
						onclick={addRoom}
						disabled={!selectedRoomId}
					>
						<Plus class="size-4" />
						<span class="sr-only">เพิ่มห้องเรียน</span>
					</Button>
				</div>
				<div class="flex flex-wrap gap-2">
					{#each details.preferredRoomIds as roomId (roomId)}
						<Badge variant="secondary" class="gap-1 py-1">
							{roomName(roomId)}
							{#if canManage}
								<button
									type="button"
									class="rounded-sm hover:text-destructive"
									onclick={() =>
										(details.preferredRoomIds = details.preferredRoomIds.filter(
											(id) => id !== roomId
										))}
									aria-label={`นำ ${roomName(roomId)} ออก`}><X class="size-3" /></button
								>
							{/if}
						</Badge>
					{:else}
						<p class="text-xs text-muted-foreground">
							ยังไม่ระบุ ให้ฝ่ายตารางสอนเลือกห้องที่ว่างได้
						</p>
					{/each}
				</div>
			</div>
			{#if canManage}
				<LoadingButton
					type="submit"
					variant="outline"
					loading={busySection === 'details'}
					loadingLabel="กำลังบันทึก"
					disabled={!details.code.trim() || !details.name.trim()}>บันทึกข้อมูลกลุ่ม</LoadingButton
				>
			{/if}
		</form>

		<div class="space-y-4 xl:border-s xl:ps-6">
			<div>
				<div class="flex flex-wrap items-center gap-2">
					<h3 class="font-medium">ครูผู้สอน</h3>
					{#if group.teachersLocked}<Badge variant="outline">ล็อกแล้ว</Badge>{/if}
				</div>
				<p class="text-xs text-muted-foreground">
					{group.teachersLocked
						? 'เผยแพร่กลุ่มเรียนแล้ว ไม่สามารถเปลี่ยนครูผู้สอนได้'
						: 'เลือกครูจากบัญชีบุคลากรและกำหนดบทบาท'}
				</p>
			</div>
			{#if canManage && !group.teachersLocked}
				<div class="space-y-2">
					<DeliveryOptionCombobox
						bind:value={selectedTeacherId}
						options={teacherOptions}
						placeholder="เลือกครูผู้สอน"
						searchPlaceholder="ค้นหาชื่อครู..."
					/>
					<div class="flex gap-2">
						<Select.Root type="single" bind:value={selectedTeacherRole}>
							<Select.Trigger class="min-w-0 flex-1" aria-label="บทบาทครูผู้สอน">
								{roleName(selectedTeacherRole)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="primary">ผู้สอนหลัก</Select.Item>
								<Select.Item value="secondary">ผู้สอนร่วม</Select.Item>
								<Select.Item value="assistant">ผู้ช่วยสอน</Select.Item>
							</Select.Content>
						</Select.Root>
						<Button
							type="button"
							variant="outline"
							onclick={addTeacher}
							disabled={!selectedTeacherId}
						>
							<Plus class="size-4" /> เพิ่ม
						</Button>
					</div>
				</div>
			{/if}
			<div class="space-y-2">
				{#each teachers as assignment (assignment.teacherId)}
					<div class="flex items-center justify-between gap-3 rounded-lg border px-3 py-2">
						<div class="min-w-0">
							<p class="truncate text-sm font-medium">{teacherName(assignment.teacherId)}</p>
							<p class="text-xs text-muted-foreground">{roleName(assignment.role)}</p>
						</div>
						{#if canManage && !group.teachersLocked}
							<Button
								type="button"
								size="icon"
								variant="ghost"
								onclick={() =>
									(teachers = teachers.filter((item) => item.teacherId !== assignment.teacherId))}
							>
								<X class="size-4" />
								<span class="sr-only">นำ {teacherName(assignment.teacherId)} ออก</span>
							</Button>
						{/if}
					</div>
				{:else}
					<div
						class="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground"
					>
						ยังไม่ได้กำหนดครูผู้สอน
					</div>
				{/each}
			</div>
			{#if canManage && !group.teachersLocked}
				<LoadingButton
					variant="outline"
					loading={busySection === 'teachers'}
					loadingLabel="กำลังบันทึก"
					onclick={() =>
						perform(
							'teachers',
							() => onReplaceTeachers({ teachers, rowVersion: group.rowVersion }),
							'บันทึกครูผู้สอนไม่สำเร็จ'
						)}>บันทึกครูผู้สอน</LoadingButton
				>
			{/if}
		</div>

		<div class="space-y-4 xl:border-s xl:ps-6">
			<div>
				<h3 class="font-medium">ห้องต้นทางของนักเรียน</h3>
				<p class="text-xs text-muted-foreground">ใช้สร้างตัวอย่างรายชื่อ ไม่ใช่ห้องที่ใช้เรียน</p>
			</div>
			{#if canManage}
				<div class="flex gap-2">
					<div class="min-w-0 flex-1">
						<DeliveryOptionCombobox
							bind:value={selectedHomeroomId}
							options={homeroomOptions.filter((homeroom) => !homeroomIds.includes(homeroom.id))}
							placeholder="เลือกห้องต้นทาง"
							searchPlaceholder="ค้นหาห้องเรียนประจำ..."
						/>
					</div>
					<Button
						type="button"
						size="icon"
						variant="outline"
						onclick={addHomeroom}
						disabled={!selectedHomeroomId}
					>
						<Plus class="size-4" />
						<span class="sr-only">เพิ่มห้องต้นทาง</span>
					</Button>
				</div>
			{/if}
			<div class="flex flex-wrap gap-2">
				{#each homeroomIds as homeroomId (homeroomId)}
					<Badge variant="secondary" class="gap-1 py-1">
						{homeroomName(homeroomId)}
						{#if canManage}
							<button
								type="button"
								class="rounded-sm hover:text-destructive"
								onclick={() => (homeroomIds = homeroomIds.filter((id) => id !== homeroomId))}
								aria-label={`นำ ${homeroomName(homeroomId)} ออก`}><X class="size-3" /></button
							>
						{/if}
					</Badge>
				{:else}
					<p class="text-sm text-muted-foreground">ยังไม่ได้เลือกห้องต้นทาง</p>
				{/each}
			</div>
			{#if canManage}
				<LoadingButton
					variant="outline"
					loading={busySection === 'homerooms'}
					loadingLabel="กำลังบันทึก"
					onclick={() =>
						perform(
							'homerooms',
							() => onReplaceHomerooms({ homeroomIds, rowVersion: group.rowVersion }),
							'บันทึกห้องต้นทางไม่สำเร็จ'
						)}>บันทึกห้องต้นทาง</LoadingButton
				>
			{/if}
		</div>
	</div>

	{#if errorMessage}
		<p role="alert" class="border-t bg-destructive/5 px-4 py-3 text-sm text-destructive">
			{errorMessage}
		</p>
	{/if}
</section>
