<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		listActivityGroups,
		listActivityMembers,
		addActivityMembers,
		removeActivityMember,
		updateMemberResult,
		ACTIVITY_TYPE_LABELS,
		type ActivityGroup,
		type ActivityGroupMember
	} from '$lib/api/academic';
	import { lookupStudents, type StudentLookupItem } from '$lib/api/lookup';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, UserPlus, Trash2, Search, Users } from 'lucide-svelte';
	import { can } from '$lib/stores/permissions';
	import { goto } from '$app/navigation';

	let groupId = $derived($page.params.id);

	// ── State ──────────────────────────────────────────
	let loading = $state(true);
	let group = $state<ActivityGroup | null>(null);
	let members = $state<ActivityGroupMember[]>([]);
	let allStudents = $state<StudentLookupItem[]>([]);

	// Add members dialog
	let showAddDialog = $state(false);
	let searchStudent = $state('');
	let selectedStudentIds = $state<string[]>([]);
	let adding = $state(false);

	// Filter members
	let memberSearch = $state('');

	// ── Computed ───────────────────────────────────────
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
		allStudents.filter((s) => {
			if (alreadyEnrolledIds.has(s.id)) return false;
			if (!searchStudent) return true;
			const q = searchStudent.toLowerCase();
			return s.name.toLowerCase().includes(q) || s.student_id?.toLowerCase().includes(q) || s.class_room?.toLowerCase().includes(q);
		}).slice(0, 80)
	);

	// ── Load ───────────────────────────────────────────
	onMount(async () => {
		await loadData();
		loading = false;
	});

	async function loadData() {
		const [groupsRes, membersRes] = await Promise.all([
			listActivityGroups({}),
			listActivityMembers(groupId)
		]);
		group = groupsRes.data.find((g) => g.id === groupId) ?? null;
		members = membersRes.data ?? [];
	}

	async function openAddDialog() {
		if (allStudents.length === 0) {
			allStudents = await lookupStudents({ limit: 5000 });
		}
		selectedStudentIds = [];
		searchStudent = '';
		showAddDialog = true;
	}

	function toggleStudent(id: string) {
		selectedStudentIds = selectedStudentIds.includes(id)
			? selectedStudentIds.filter((x) => x !== id)
			: [...selectedStudentIds, id];
	}

	async function handleAdd() {
		if (selectedStudentIds.length === 0) { toast.error('กรุณาเลือกนักเรียน'); return; }
		adding = true;
		try {
			const res: any = await addActivityMembers(groupId, selectedStudentIds);
			toast.success(`เพิ่มสมาชิก ${res.inserted ?? selectedStudentIds.length} คนแล้ว`);
			showAddDialog = false;
			await loadData();
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		} finally {
			adding = false;
		}
	}

	async function handleRemove(member: ActivityGroupMember) {
		try {
			await removeActivityMember(groupId, member.student_id);
			toast.success('ลบสมาชิกแล้ว');
			await loadData();
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		}
	}

	async function handleResultChange(member: ActivityGroupMember, result: 'pass' | 'fail' | '') {
		if (!result) return;
		try {
			await updateMemberResult(member.id, result as 'pass' | 'fail');
			toast.success('บันทึกผลแล้ว');
			await loadData();
		} catch {
			toast.error('เกิดข้อผิดพลาด');
		}
	}
</script>

<div class="space-y-4 p-4">
	<!-- Back + Header -->
	<div class="flex items-center gap-3">
		<Button variant="ghost" size="icon" onclick={() => goto('/staff/academic/activities')}>
			<ArrowLeft class="h-4 w-4" />
		</Button>
		<div>
			{#if group}
				<h1 class="text-xl font-semibold">{group.name}</h1>
				<p class="text-sm text-muted-foreground">
					<Badge variant="outline">{ACTIVITY_TYPE_LABELS[group.activity_type] ?? group.activity_type}</Badge>
					{#if group.instructor_name}
						· ครูที่ดูแล: {group.instructor_name}
					{/if}
					{#if group.max_capacity}
						· รับสูงสุด {group.max_capacity} คน
					{/if}
					· {group.member_count ?? members.length} คน
				</p>
			{:else}
				<h1 class="text-xl font-semibold">กลุ่มกิจกรรม</h1>
			{/if}
		</div>
	</div>

	<!-- Toolbar -->
	<div class="flex items-center justify-between gap-3">
		<div class="relative">
			<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
			<Input class="pl-8 w-56" placeholder="ค้นหาสมาชิก..." bind:value={memberSearch} />
		</div>
		{#if $can.has('activity.members.manage')}
			<Button onclick={openAddDialog}>
				<UserPlus class="mr-1 h-4 w-4" />
				เพิ่มสมาชิก
			</Button>
		{/if}
	</div>

	<!-- Members Table -->
	{#if loading}
		<p class="text-muted-foreground text-sm">กำลังโหลด...</p>
	{:else if filteredMembers.length === 0}
		<p class="text-muted-foreground text-sm">ยังไม่มีสมาชิก</p>
	{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>ชื่อ-นามสกุล</Table.Head>
					<Table.Head>รหัสนักเรียน</Table.Head>
					<Table.Head>ห้องเรียน</Table.Head>
					<Table.Head>ระดับชั้น</Table.Head>
					<Table.Head class="text-center">ผล (ผ/มผ)</Table.Head>
					{#if $can.has('activity.members.manage')}
						<Table.Head></Table.Head>
					{/if}
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each filteredMembers as m}
					<Table.Row>
						<Table.Cell class="font-medium">{m.student_name ?? '—'}</Table.Cell>
						<Table.Cell class="text-sm text-muted-foreground">{m.student_code ?? '—'}</Table.Cell>
						<Table.Cell class="text-sm">{m.classroom_name ?? '—'}</Table.Cell>
						<Table.Cell class="text-sm">{m.grade_level_name ?? '—'}</Table.Cell>
						<Table.Cell class="text-center">
							{#if $can.has('activity.members.manage')}
							<select
								class="h-7 rounded border px-1 text-xs bg-background"
								value={m.result ?? ''}
								onchange={(e) => handleResultChange(m, (e.target as HTMLSelectElement).value as any)}
							>
								<option value="">—</option>
								<option value="pass">ผ (ผ่าน)</option>
								<option value="fail">มผ (ไม่ผ่าน)</option>
							</select>
							{:else}
								{m.result === 'pass' ? 'ผ' : m.result === 'fail' ? 'มผ' : '—'}
							{/if}
						</Table.Cell>
						{#if $can.has('activity.members.manage')}
							<Table.Cell>
								<Button
									variant="ghost"
									size="icon"
									onclick={() => handleRemove(m)}
								>
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

<!-- Add Members Dialog -->
<Dialog.Root bind:open={showAddDialog}>
	<Dialog.Content class="max-w-lg">
		<Dialog.Header>
			<Dialog.Title>เพิ่มสมาชิก</Dialog.Title>
			<Dialog.Description>
				เลือกนักเรียนที่ต้องการเพิ่มเข้ากลุ่ม
				{#if selectedStudentIds.length > 0}
					· เลือก {selectedStudentIds.length} คน
				{/if}
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-3">
			<div class="relative">
				<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
				<Input class="pl-8" placeholder="ค้นหาชื่อ รหัส หรือห้องเรียน..." bind:value={searchStudent} />
			</div>

			<div class="max-h-72 overflow-y-auto divide-y rounded border">
				{#each filteredStudents as s}
					{@const checked = selectedStudentIds.includes(s.id)}
					<button
						type="button"
						class="flex w-full items-center gap-3 px-3 py-2 text-sm hover:bg-accent text-left"
						onclick={() => toggleStudent(s.id)}
					>
						<div class="flex h-4 w-4 items-center justify-center rounded border {checked ? 'bg-primary border-primary' : 'border-input'}">
							{#if checked}<span class="text-primary-foreground text-xs">✓</span>{/if}
						</div>
						<div class="flex-1">
							<span class="font-medium">{s.name}</span>
							{#if s.student_id || s.class_room}
								<span class="text-muted-foreground ml-2">{s.student_id ?? ''} {s.class_room ? `· ${s.class_room}` : ''}</span>
							{/if}
						</div>
					</button>
				{:else}
					<p class="px-3 py-4 text-sm text-muted-foreground">ไม่พบนักเรียน</p>
				{/each}
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" onclick={() => { showAddDialog = false; }}>ยกเลิก</Button>
			<Button onclick={handleAdd} disabled={adding || selectedStudentIds.length === 0}>
				{adding ? 'กำลังเพิ่ม...' : `เพิ่ม ${selectedStudentIds.length} คน`}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
