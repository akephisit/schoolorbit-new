<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { untrack } from 'svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, RegionUpdatingState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import type { HomeroomRoster, HomeroomRosterStudent } from '#lib/api/homeroom-roster.js';
	import HomeroomRosterDialogs, {
		type RosterDialogMode
	} from '#lib/components/academic-core/HomeroomRosterDialogs.svelte';
	import { Plus, ArrowDownAZ, MoreHorizontal, ArrowRightLeft } from '@lucide/svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let roster = $state<HomeroomRoster | null>(null);
	let loading = $state(true);
	let errorMessage = $state('');
	let search = $state('');
	let selected = $state<string[]>([]);
	let mode = $state<RosterDialogMode>(null);
	let actionStudents = $state<HomeroomRosterStudent[]>([]);
	let contextKey = '';
	let mutationRevision = 0;
	const backHref = $derived(
		`/staff/academic/homerooms?academicYearId=${data.academicYearId ?? ''}`
	);
	const canManage = $derived(
		$can.has(PERMISSIONS.STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL) &&
			Boolean(roster?.homeroom.isActive) &&
			!['closed', 'archived'].includes(roster?.yearStatus ?? 'closed')
	);
	const visible = $derived(
		roster?.students.filter((student) =>
			`${student.firstName} ${student.lastName} ${student.studentCode ?? ''}`
				.toLocaleLowerCase('th')
				.includes(search.trim().toLocaleLowerCase('th'))
		) ?? []
	);
	const selectedStudents = $derived(
		roster?.students.filter((student) => selected.includes(student.placementId)) ?? []
	);
	const allVisibleSelected = $derived(
		visible.length > 0 && visible.every((student) => selected.includes(student.placementId))
	);
	const missingNumbers = $derived(
		roster?.students.filter((student) => student.classNumber == null).length ?? 0
	);

	$effect.pre(() => {
		const result = data.roster;
		const key = `${data.roomId}:${data.academicYearId}`;
		const startedRevision = untrack(() => mutationRevision);
		let current = true;
		untrack(() => {
			if (contextKey !== key) {
				contextKey = key;
				roster = null;
				selected = [];
				mode = null;
				search = '';
				actionStudents = [];
			}
			loading = Boolean(result);
			errorMessage = '';
		});
		if (result)
			void result.then((outcome) => {
				if (!current) return;
				untrack(() => {
					if (outcome.ok && startedRevision === mutationRevision) {
						roster = outcome.data;
						selected = [];
					} else if (!outcome.ok) errorMessage = outcome.error;
					loading = false;
				});
			});
		return () => {
			current = false;
		};
	});
	function toggle(id: string, checked: boolean) {
		selected = checked ? [...new Set([...selected, id])] : selected.filter((value) => value !== id);
	}
	function toggleVisible(checked: boolean) {
		const ids = visible.map((student) => student.placementId);
		selected = checked
			? [...new Set([...selected, ...ids])]
			: selected.filter((id) => !ids.includes(id));
	}
	function open(next: RosterDialogMode, students: HomeroomRosterStudent[] = []) {
		if (!canManage) return;
		actionStudents = [...students];
		mode = next;
	}
	function saved(updated: HomeroomRoster) {
		if (
			updated.homeroom.id !== data.roomId ||
			updated.homeroom.academicYearId !== data.academicYearId
		)
			return;
		mutationRevision += 1;
		roster = updated;
		selected = [];
		errorMessage = '';
	}
	const refresh = () => invalidate('schoolorbit:homeroom-roster');
</script>

<PageShell
	title={roster ? `นักเรียนในห้อง ${roster.homeroom.name}` : 'นักเรียนในห้อง'}
	description="จัดนักเรียนเข้าห้อง ย้ายห้อง และกำหนดเลขที่ประจำห้อง"
	{backHref}
>
	{#if !data.academicYearId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาก่อน"
			description="เลือกปีจากแถบด้านบน แล้วเปิดห้องที่ต้องการ"
		/>
	{:else if loading && !roster}
		<PageSkeleton variant="table" rows={8} />
	{:else if !roster}
		<PageState
			variant="error"
			title="โหลดนักเรียนในห้องไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={refresh}
		/>
	{:else}
		<div class="relative space-y-4" aria-busy={loading} data-testid="homeroom-roster-ready">
			{#if loading}<RegionUpdatingState label="กำลังอัปเดตรายชื่อนักเรียน" />{/if}
			<div
				class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card p-3 sm:p-4"
			>
				<div>
					<p class="font-semibold">{roster.students.length} / {roster.homeroom.capacity} คน</p>
					<p class="text-sm text-muted-foreground">
						{missingNumbers
							? `ยังไม่มีเลขที่ ${missingNumbers} คน`
							: 'เรียงรายชื่อตามเลขที่ประจำห้อง'}
					</p>
				</div>
				<Button href={backHref} variant="outline" size="sm">เปลี่ยนห้อง</Button>
			</div>
			{#if !canManage}<p class="text-sm text-muted-foreground">
					{['closed', 'archived'].includes(roster.yearStatus)
						? 'ปีการศึกษานี้ปิดแล้ว ดูข้อมูลย้อนหลังได้'
						: 'เปิดดูรายชื่อได้ การแก้ไขต้องใช้สิทธิ์จัดการนักเรียนประจำปี'}
				</p>{/if}
			<div class="flex flex-wrap gap-3">
				{#if canManage}
					<Button onclick={() => open('add')} disabled={loading}
						><Plus class="size-4" />เพิ่มนักเรียนเข้าห้อง</Button
					>
					<Button
						variant="outline"
						onclick={() => open('renumber')}
						disabled={loading || roster.students.length === 0}
						><ArrowDownAZ class="size-4" />จัดเลขที่</Button
					>
				{/if}
				<Button variant="ghost" onclick={refresh} disabled={loading}>รีเฟรชรายชื่อ</Button>
			</div>
			<div class="rounded-xl border bg-card p-3 sm:p-4">
				<Input
					aria-label="ค้นหานักเรียนในห้อง"
					bind:value={search}
					placeholder="ค้นหาชื่อหรือรหัสนักเรียน…"
				/>
			</div>
			{#if canManage && selected.length > 0}
				<div
					class="flex flex-wrap items-center gap-3 rounded-xl border bg-muted/30 p-3"
					role="status"
				>
					<span class="text-sm">เลือก {selected.length} คน</span>
					<Button
						size="sm"
						variant="outline"
						onclick={() => open('transfer', selectedStudents)}
						disabled={loading}><ArrowRightLeft class="size-4" />ย้ายไปห้องอื่น</Button
					>
					<Button
						size="sm"
						variant="outline"
						onclick={() => open('remove', selectedStudents)}
						disabled={loading}>นำออกจากห้อง</Button
					>
					<Button size="sm" variant="ghost" onclick={() => (selected = [])}>ยกเลิกการเลือก</Button>
				</div>
			{/if}
			<div class="overflow-x-auto rounded-xl border bg-card">
				<Table.Root>
					<Table.Header
						><Table.Row>
							{#if canManage}<Table.Head class="w-12"
									><Checkbox
										aria-label="เลือกนักเรียนที่แสดงทั้งหมด"
										checked={allVisibleSelected}
										onCheckedChange={toggleVisible}
										disabled={loading}
									/></Table.Head
								>{/if}
							<Table.Head class="w-20">เลขที่</Table.Head><Table.Head class="min-w-28"
								>รหัสนักเรียน</Table.Head
							><Table.Head class="min-w-56">ชื่อ–นามสกุล</Table.Head><Table.Head class="min-w-28"
								>สถานะห้อง</Table.Head
							><Table.Head class="w-24">จัดการ</Table.Head>
						</Table.Row></Table.Header
					>
					<Table.Body>
						{#each visible as student (student.placementId)}
							<Table.Row>
								{#if canManage}<Table.Cell
										><Checkbox
											aria-label={`เลือก ${student.firstName} ${student.lastName}`}
											checked={selected.includes(student.placementId)}
											onCheckedChange={(checked) => toggle(student.placementId, checked)}
											disabled={loading}
										/></Table.Cell
									>{/if}
								<Table.Cell class="tabular-nums">{student.classNumber ?? '—'}</Table.Cell>
								<Table.Cell class="font-mono text-xs"
									>{student.studentCode ?? 'ยังไม่มีรหัส'}</Table.Cell
								>
								<Table.Cell
									><p class="font-medium">
										{student.title ?? ''}{student.firstName}
										{student.lastName}
									</p>
									{#if student.classNumber == null}<p class="text-xs text-muted-foreground">
											ยังไม่ได้กำหนดเลขที่
										</p>{/if}</Table.Cell
								>
								<Table.Cell class="text-sm text-muted-foreground"
									>{student.status === 'planned' ? 'เตรียมการ' : 'ห้องปัจจุบัน'}</Table.Cell
								>
								<Table.Cell>
									{#if canManage}<DropdownMenu.Root>
											<DropdownMenu.Trigger
												aria-label={`จัดการ ${student.firstName}`}
												class="rounded-md p-2 hover:bg-muted"
												><MoreHorizontal class="size-4" /></DropdownMenu.Trigger
											>
											<DropdownMenu.Content
												><DropdownMenu.Item onclick={() => open('number', [student])}
													>แก้เลขที่</DropdownMenu.Item
												><DropdownMenu.Item onclick={() => open('transfer', [student])}
													>ย้ายห้อง</DropdownMenu.Item
												><DropdownMenu.Item onclick={() => open('remove', [student])}
													>นำออกจากห้อง</DropdownMenu.Item
												></DropdownMenu.Content
											>
										</DropdownMenu.Root>{/if}
									{#if $can.hasAny(PERMISSIONS.HOMEROOM_READ_SCHOOL, PERMISSIONS.HOMEROOM_MANAGE_SCHOOL)}<Button
											size="sm"
											variant="ghost"
											data-sveltekit-preload-data="tap"
											href={`/staff/academic/student-years?academicYearId=${data.academicYearId}&studentYearId=${student.studentAcademicYearId}`}
											>ประวัติ</Button
										>{/if}
								</Table.Cell>
							</Table.Row>
						{:else}<Table.Row
								><Table.Cell
									colspan={canManage ? 6 : 5}
									class="h-32 text-center text-muted-foreground"
									>{search
										? 'ไม่พบนักเรียนที่ตรงกับคำค้นหา'
										: 'ห้องนี้ยังไม่มีนักเรียน'}</Table.Cell
								></Table.Row
							>{/each}
					</Table.Body>
				</Table.Root>
			</div>
			{#if errorMessage}<div role="alert" class="flex items-center gap-3 text-sm text-destructive">
					<span>{errorMessage}</span><Button size="sm" variant="outline" onclick={refresh}
						>ลองอีกครั้ง</Button
					>
				</div>{/if}
		</div>
		{#key roster.homeroom.id}<HomeroomRosterDialogs
				{roster}
				bind:mode
				students={actionStudents}
				onSaved={saved}
			/>{/key}
	{/if}
</PageShell>
