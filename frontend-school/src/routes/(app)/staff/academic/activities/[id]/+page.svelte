<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		listActivityGroups,
		listActivityMembers,
		addActivityMembers,
		removeActivityMember,
		updateMemberResult,
		listActivityInstructors,
		addActivityInstructor,
		removeActivityInstructor,
		ACTIVITY_TYPE_LABELS,
		type ActivityGroup,
		type ActivityGroupMember,
		type ActivityInstructor
	} from '$lib/api/academic';
	import {
		lookupStudents,
		lookupStaff,
		type StudentLookupItem,
		type StaffLookupItem
	} from '$lib/api/lookup';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import * as Tabs from '$lib/components/ui/tabs';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, UserPlus, Trash2, Search, UserCog } from 'lucide-svelte';
	import { can } from '$lib/stores/permissions';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	let groupId = $derived($page.params.id as string);

	let loading = $state(true);
	let group = $state<ActivityGroup | null>(null);
	let members = $state<ActivityGroupMember[]>([]);
	let instructors = $state<ActivityInstructor[]>([]);
	let activeTab = $state('members');

	let allStudents = $state<StudentLookupItem[]>([]);
	let showAddStudentDialog = $state(false);
	let searchStudent = $state('');
	let selectedStudentIds = $state<string[]>([]);
	let adding = $state(false);

	let allStaff = $state<StaffLookupItem[]>([]);
	let showAddInstructorDialog = $state(false);
	let addInstructorId = $state('');
	let addInstructorRole = $state('assistant');

	let memberSearch = $state('');
	let filteredMembers = $derived(
		members.filter((m) => {
			if (!memberSearch) return true;
			const q = memberSearch.toLowerCase();
			return (
				m.student_name?.toLowerCase().includes(q) ||
				m.student_code?.toLowerCase().includes(q) ||
				m.classroom_name?.toLowerCase().includes(q)
			);
		})
	);
	let alreadyEnrolledIds = $derived(new Set(members.map((m) => m.student_id)));
	let filteredStudents = $derived(
		allStudents
			.filter((s) => {
				if (alreadyEnrolledIds.has(s.id)) return false;
				if (!searchStudent) return true;
				const q = searchStudent.toLowerCase();
				return (
					s.name.toLowerCase().includes(q) ||
					s.student_id?.toLowerCase().includes(q) ||
					s.class_room?.toLowerCase().includes(q)
				);
			})
			.slice(0, 80)
	);

	let addInstructorName = $derived(
		allStaff.find((s) => s.id === addInstructorId)?.name ?? 'เลือกครู...'
	);

	onMount(async () => {
		await loadAll();
		loading = false;
	});

	async function loadAll() {
		const [groupsRes, membersRes, instructorsRes] = await Promise.all([
			listActivityGroups({}),
			listActivityMembers(groupId),
			listActivityInstructors(groupId)
		]);
		group = groupsRes.data.find((g) => g.id === groupId) ?? null;
		members = membersRes.data ?? [];
		instructors = instructorsRes.data ?? [];
	}

	async function openAddStudentDialog() {
		if (allStudents.length === 0) allStudents = await lookupStudents({ limit: 5000 });
		selectedStudentIds = [];
		searchStudent = '';
		showAddStudentDialog = true;
	}
	function toggleStudent(id: string) {
		selectedStudentIds = selectedStudentIds.includes(id)
			? selectedStudentIds.filter((x) => x !== id)
			: [...selectedStudentIds, id];
	}
	async function handleAddStudents() {
		if (!selectedStudentIds.length) {
			toast.error('กรุณาเลือกนักเรียน');
			return;
		}
		adding = true;
		try {
			const res = (await addActivityMembers(groupId, selectedStudentIds)) as {
				inserted?: number;
			};
			toast.success(`เพิ่มสมาชิก ${res.inserted ?? selectedStudentIds.length} คนแล้ว`);
			showAddStudentDialog = false;
			members = (await listActivityMembers(groupId)).data ?? [];
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		} finally {
			adding = false;
		}
	}
	async function handleRemoveMember(m: ActivityGroupMember) {
		try {
			await removeActivityMember(groupId, m.student_id);
			toast.success('ลบสมาชิกแล้ว');
			members = (await listActivityMembers(groupId)).data ?? [];
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		}
	}
	async function handleResultChange(m: ActivityGroupMember, val: string) {
		if (!val) return;
		try {
			await updateMemberResult(m.id, val as 'pass' | 'fail');
			toast.success('บันทึกผลแล้ว');
			members = (await listActivityMembers(groupId)).data ?? [];
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		}
	}

	async function openAddInstructorDialog() {
		if (!allStaff.length) allStaff = await lookupStaff({ activeOnly: true, limit: 1000 });
		addInstructorId = '';
		addInstructorRole = 'assistant';
		showAddInstructorDialog = true;
	}
	async function handleAddInstructor() {
		if (!addInstructorId) {
			toast.error('กรุณาเลือกครู');
			return;
		}
		try {
			await addActivityInstructor(
				groupId,
				addInstructorId,
				addInstructorRole as 'primary' | 'assistant'
			);
			toast.success('เพิ่มครูแล้ว');
			showAddInstructorDialog = false;
			instructors = (await listActivityInstructors(groupId)).data ?? [];
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		}
	}
	async function handleRemoveInstructor(i: ActivityInstructor) {
		try {
			await removeActivityInstructor(groupId, i.instructor_id);
			toast.success('ลบครูแล้ว');
			instructors = (await listActivityInstructors(groupId)).data ?? [];
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		}
	}
</script>

<div class="space-y-4 p-4">
	<div class="flex items-center gap-3">
		<Button variant="ghost" size="icon" onclick={() => goto(resolve('/staff/academic/activities'))}>
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div>
			{#if group}
				<h1 class="text-xl font-semibold">{group.name}</h1>
				<p class="text-sm text-muted-foreground flex items-center gap-2 flex-wrap">
					{#if group.activity_type}<Badge variant="outline"
							>{ACTIVITY_TYPE_LABELS[group.activity_type] ?? group.activity_type}</Badge
						>{/if}
					· {members.length} สมาชิก{#if group.max_capacity}
						· รับสูงสุด {group.max_capacity} คน{/if}
				</p>
			{:else}
				<h1 class="text-xl font-semibold">กลุ่มกิจกรรม</h1>
			{/if}
		</div>
	</div>

	<Tabs.Root bind:value={activeTab}>
		<Tabs.List>
			<Tabs.Trigger value="members">สมาชิก ({members.length})</Tabs.Trigger>
			<Tabs.Trigger value="instructors">ครูที่ดูแล ({instructors.length})</Tabs.Trigger>
		</Tabs.List>

		<!-- Members -->
		<Tabs.Content value="members">
			<div class="space-y-3 pt-3">
				<div class="flex items-center justify-between gap-3">
					<div class="relative">
						<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
						<Input class="pl-8 w-56" placeholder="ค้นหา..." bind:value={memberSearch} />
					</div>
					{#if $can.has('activity.members.manage')}
						<Button onclick={openAddStudentDialog}
							><UserPlus class="mr-1 h-4 w-4" />เพิ่มสมาชิก</Button
						>
					{/if}
				</div>
				{#if loading}
					<p class="text-sm text-muted-foreground">กำลังโหลด...</p>
				{:else if filteredMembers.length === 0}
					<p class="text-sm text-muted-foreground">ยังไม่มีสมาชิก</p>
				{:else}
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head>ชื่อ-นามสกุล</Table.Head>
								<Table.Head>รหัส</Table.Head>
								<Table.Head>ห้องเรียน</Table.Head>
								<Table.Head>ระดับชั้น</Table.Head>
								<Table.Head class="text-center">ผล</Table.Head>
								{#if $can.has('activity.members.manage')}<Table.Head></Table.Head>{/if}
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each filteredMembers as m (m.id)}
								<Table.Row>
									<Table.Cell class="font-medium">{m.student_name ?? '—'}</Table.Cell>
									<Table.Cell class="text-sm text-muted-foreground"
										>{m.student_code ?? '—'}</Table.Cell
									>
									<Table.Cell class="text-sm">{m.classroom_name ?? '—'}</Table.Cell>
									<Table.Cell class="text-sm">{m.grade_level_name ?? '—'}</Table.Cell>
									<Table.Cell class="text-center">
										{#if $can.has('activity.members.manage')}
											<select
												class="h-7 rounded border px-1 text-xs bg-background"
												value={m.result ?? ''}
												onchange={(e) =>
													handleResultChange(m, (e.target as HTMLSelectElement).value)}
											>
												<option value="">—</option>
												<option value="pass">ผ</option>
												<option value="fail">มผ</option>
											</select>
										{:else}
											{m.result === 'pass' ? 'ผ' : m.result === 'fail' ? 'มผ' : '—'}
										{/if}
									</Table.Cell>
									{#if $can.has('activity.members.manage')}
										<Table.Cell>
											<Button variant="ghost" size="icon" onclick={() => handleRemoveMember(m)}>
												<Trash2 class="h-4 w-4 text-destructive" />
											</Button>
										</Table.Cell>
									{/if}
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				{/if}
			</div>
		</Tabs.Content>

		<!-- Instructors -->
		<Tabs.Content value="instructors">
			<div class="space-y-3 pt-3">
				{#if $can.has('activity.manage.all') || $can.has('activity.manage.own')}
					<div class="flex justify-end">
						<Button onclick={openAddInstructorDialog}
							><UserCog class="mr-1 h-4 w-4" />เพิ่มครู</Button
						>
					</div>
				{/if}
				{#if instructors.length === 0}
					<p class="text-sm text-muted-foreground">ยังไม่มีครูที่ดูแล</p>
				{:else}
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head>ชื่อครู</Table.Head>
								<Table.Head>บทบาท</Table.Head>
								{#if $can.has('activity.manage.all') || $can.has('activity.manage.own')}<Table.Head
									></Table.Head>{/if}
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each instructors as i (i.id)}
								<Table.Row>
									<Table.Cell class="font-medium">{i.instructor_name ?? '—'}</Table.Cell>
									<Table.Cell>
										<Badge variant={i.role === 'primary' ? 'default' : 'outline'}>
											{i.role === 'primary' ? 'ครูหลัก' : 'ครูผู้ช่วย'}
										</Badge>
									</Table.Cell>
									{#if $can.has('activity.manage.all') || $can.has('activity.manage.own')}
										<Table.Cell>
											<Button variant="ghost" size="icon" onclick={() => handleRemoveInstructor(i)}>
												<Trash2 class="h-4 w-4 text-destructive" />
											</Button>
										</Table.Cell>
									{/if}
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				{/if}
			</div>
		</Tabs.Content>
	</Tabs.Root>
</div>

<!-- Add Student Dialog -->
<Dialog.Root bind:open={showAddStudentDialog}>
	<Dialog.Content class="max-w-lg">
		<Dialog.Header>
			<Dialog.Title>เพิ่มสมาชิก</Dialog.Title>
			<Dialog.Description
				>เลือกนักเรียน{#if selectedStudentIds.length > 0}
					· เลือก {selectedStudentIds.length} คน{/if}</Dialog.Description
			>
		</Dialog.Header>
		<div class="space-y-3">
			<div class="relative">
				<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
				<Input class="pl-8" placeholder="ค้นหา..." bind:value={searchStudent} />
			</div>
			<div class="max-h-72 overflow-y-auto divide-y rounded border">
				{#each filteredStudents as s (s.id)}
					{@const checked = selectedStudentIds.includes(s.id)}
					<button
						type="button"
						class="flex w-full items-center gap-3 px-3 py-2 text-sm hover:bg-accent text-left"
						onclick={() => toggleStudent(s.id)}
					>
						<div
							class="flex h-4 w-4 items-center justify-center rounded border {checked
								? 'bg-primary border-primary'
								: 'border-input'}"
						>
							{#if checked}<span class="text-primary-foreground text-xs">✓</span>{/if}
						</div>
						<div class="flex-1">
							<span class="font-medium">{s.name}</span>
							{#if s.student_id || s.class_room}
								<span class="text-muted-foreground ml-2"
									>{s.student_id ?? ''}{s.class_room ? ` · ${s.class_room}` : ''}</span
								>
							{/if}
						</div>
					</button>
				{:else}
					<p class="px-3 py-4 text-sm text-muted-foreground">ไม่พบนักเรียน</p>
				{/each}
			</div>
		</div>
		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => {
					showAddStudentDialog = false;
				}}>ยกเลิก</Button
			>
			<Button onclick={handleAddStudents} disabled={adding || selectedStudentIds.length === 0}>
				{adding ? 'กำลังเพิ่ม...' : `เพิ่ม ${selectedStudentIds.length} คน`}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<!-- Add Instructor Dialog -->
<Dialog.Root bind:open={showAddInstructorDialog}>
	<Dialog.Content class="max-w-sm">
		<Dialog.Header><Dialog.Title>เพิ่มครูที่ดูแล</Dialog.Title></Dialog.Header>
		<div class="space-y-3 py-2">
			<div class="space-y-1">
				<Label>ครู</Label>
				<Select.Root type="single" bind:value={addInstructorId}>
					<Select.Trigger class="w-full">{addInstructorName}</Select.Trigger>
					<Select.Content class="max-h-56 overflow-y-auto">
						{#each allStaff as s (s.id)}
							<Select.Item value={s.id}>{s.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="space-y-1">
				<Label>บทบาท</Label>
				<Select.Root type="single" bind:value={addInstructorRole}>
					<Select.Trigger class="w-full"
						>{addInstructorRole === 'primary' ? 'ครูหลัก' : 'ครูผู้ช่วย'}</Select.Trigger
					>
					<Select.Content>
						<Select.Item value="primary">ครูหลัก</Select.Item>
						<Select.Item value="assistant">ครูผู้ช่วย</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>
		</div>
		<Dialog.Footer>
			<Button
				variant="outline"
				onclick={() => {
					showAddInstructorDialog = false;
				}}>ยกเลิก</Button
			>
			<Button onclick={handleAddInstructor}>เพิ่ม</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
