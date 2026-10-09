<script lang="ts" module>
	export type RosterDialogMode = 'add' | 'transfer' | 'remove' | 'renumber' | 'number' | null;
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		listHomeroomTransferTargets,
		listHomeroomRosterCandidates,
		previewHomeroomNumbers,
		updateHomeroomNumbers,
		mutateHomeroomRoster,
		type HomeroomRoster,
		type HomeroomRosterStudent,
		type HomeroomRosterCandidate,
		type HomeroomNumberingMethod,
		type HomeroomNumberingPreview,
		type HomeroomRosterSelection
	} from '#lib/api/homeroom-roster.js';
	import { type Homeroom } from '#lib/api/academic-core.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { PageState, PageSkeleton } from '#lib/components/app-state/index.js';

	let {
		roster,
		mode = $bindable(null),
		students,
		onSaved
	}: {
		roster: HomeroomRoster;
		mode: RosterDialogMode;
		students: HomeroomRosterStudent[];
		onSaved: (roster: HomeroomRoster) => void;
	} = $props();
	let busy = $state(false),
		reading = $state(false),
		errorMessage = $state('');
	let search = $state(''),
		candidateIds = $state<string[]>([]),
		candidates = $state<HomeroomRosterCandidate[]>([]);
	let rooms = $state<Homeroom[]>([]),
		targetId = $state(''),
		effectiveDate = $state(''),
		reason = $state('');
	let method = $state<HomeroomNumberingMethod>('name'),
		startNumber = $state<number | undefined>(1);
	let preview = $state<HomeroomNumberingPreview | null>(null),
		number = $state<number | undefined>(1);
	let sourceRevision = '';
	const request = new LatestRequest();
	const numberingMethods: { value: HomeroomNumberingMethod; label: string }[] = [
		{ value: 'name', label: 'ตามชื่อ ก–ฮ' },
		{ value: 'student_code', label: 'ตามรหัสนักเรียน' },
		{ value: 'gender_name', label: 'ชายก่อน–หญิงตาม แล้วเรียงชื่อ' }
	];
	const titles = {
		add: 'เพิ่มนักเรียนเข้าห้อง',
		transfer: 'ย้ายไปห้องอื่น',
		remove: 'นำออกจากห้อง',
		renumber: 'จัดเลขที่อัตโนมัติ',
		number: 'แก้เลขที่นักเรียน'
	};
	const title = $derived(mode ? titles[mode] : 'จัดการนักเรียน');
	const changedCount = $derived(
		preview?.numbers.filter(
			(input) =>
				preview?.roster.students.find((student) => student.placementId === input.placementId)
					?.classNumber !== input.classNumber
		).length ?? 0
	);

	$effect(() => {
		const nextMode = mode;
		untrack(() => {
			request.abort();
			reading = false;
			errorMessage = '';
			preview = null;
			candidateIds = [];
			candidates = [];
			rooms = [];
			search = '';
			reason = '';
			targetId = '';
			sourceRevision = roster.revision;
			effectiveDate = new Intl.DateTimeFormat('en-CA', {
				timeZone: 'Asia/Bangkok',
				year: 'numeric',
				month: '2-digit',
				day: '2-digit'
			}).format(new Date());
			number = students[0]?.classNumber ?? 1;
			startNumber = 1;
			method = 'name';
			if (nextMode === 'add') void loadCandidates();
			if (nextMode === 'transfer') void loadRooms();
		});
		return () => request.abort();
	});
	function close() {
		if (!busy) mode = null;
	}
	async function loadCandidates() {
		const { revision, signal } = request.begin();
		reading = true;
		errorMessage = '';
		try {
			const rows = await listHomeroomRosterCandidates(roster.homeroom.id, search, { signal });
			if (request.isCurrent(revision)) {
				// Retain selected rows while searching another page of candidates.
				candidates = [
					...candidates.filter(
						(row) =>
							candidateIds.includes(row.studentAcademicYearId) &&
							!rows.some(
								(candidate) => candidate.studentAcademicYearId === row.studentAcademicYearId
							)
					),
					...rows
				];
			}
		} catch (error) {
			if (!isAbortError(error) && request.isCurrent(revision))
				errorMessage = error instanceof Error ? error.message : 'ค้นหานักเรียนไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) reading = false;
		}
	}
	async function loadRooms() {
		const { revision, signal } = request.begin();
		reading = true;
		try {
			const rows = await listHomeroomTransferTargets(roster.homeroom.id, { signal });
			if (request.isCurrent(revision))
				rooms = rows.filter(
					(room) =>
						room.id !== roster.homeroom.id &&
						room.isActive &&
						room.gradeLevelId === roster.homeroom.gradeLevelId &&
						room.studyProgramId === roster.homeroom.studyProgramId
				);
		} catch (error) {
			if (!isAbortError(error) && request.isCurrent(revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดห้องไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) reading = false;
		}
	}
	function clearPreview() {
		request.abort();
		reading = false;
		preview = null;
		errorMessage = '';
	}
	async function loadPreview() {
		if (!Number.isInteger(startNumber) || !startNumber || startNumber < 1) {
			errorMessage = 'เลขที่เริ่มต้นต้องเป็นจำนวนเต็มบวก';
			return;
		}
		const { revision, signal } = request.begin();
		reading = true;
		errorMessage = '';
		preview = null;
		try {
			const result = await previewHomeroomNumbers(roster.homeroom.id, method, startNumber, {
				signal
			});
			if (request.isCurrent(revision)) preview = result;
		} catch (error) {
			if (!isAbortError(error) && request.isCurrent(revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดตัวอย่างไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) reading = false;
		}
	}
	function toggleCandidate(id: string, checked: boolean) {
		candidateIds = checked
			? [...new Set([...candidateIds, id])]
			: candidateIds.filter((value) => value !== id);
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!mode || busy) return;
		const roomId = roster.homeroom.id;
		busy = true;
		errorMessage = '';
		try {
			let updated: HomeroomRoster;
			if (mode === 'renumber') {
				if (!preview) throw new Error('กดดูตัวอย่างก่อนบันทึก');
				updated = await updateHomeroomNumbers(roomId, preview.roster.revision, preview.numbers);
			} else if (mode === 'number') {
				if (!students[0] || !Number.isInteger(number) || !number || number < 1)
					throw new Error('เลขที่ต้องเป็นจำนวนเต็มบวก');
				updated = await updateHomeroomNumbers(roomId, sourceRevision, [
					{ placementId: students[0].placementId, classNumber: number }
				]);
			} else {
				let selections: HomeroomRosterSelection[];
				if (mode === 'add') {
					selections = candidates
						.filter((candidate) => candidateIds.includes(candidate.studentAcademicYearId))
						.map((candidate) => ({
							studentAcademicYearId: candidate.studentAcademicYearId,
							studentYearRowVersion: candidate.rowVersion,
							placementId: null,
							placementRowVersion: null
						}));
				} else
					selections = students.map((student) => ({
						studentAcademicYearId: student.studentAcademicYearId,
						studentYearRowVersion: student.studentYearRowVersion,
						placementId: student.placementId,
						placementRowVersion: student.rowVersion
					}));
				if (selections.length === 0) throw new Error('กรุณาเลือกนักเรียน');
				if (!effectiveDate) throw new Error('กรุณาเลือกวันที่');
				if (mode === 'transfer' && !targetId) throw new Error('กรุณาเลือกห้องปลายทาง');
				if (mode !== 'add' && !reason.trim()) throw new Error('กรุณาระบุเหตุผล');
				updated = await mutateHomeroomRoster(roomId, {
					revision: sourceRevision,
					action: mode,
					selections,
					effectiveDate,
					targetHomeroomId: mode === 'transfer' ? targetId : null,
					reason
				});
			}
			onSaved(updated);
			toast.success('บันทึกการจัดนักเรียนเรียบร้อยแล้ว');
			mode = null;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<Dialog.Root
	open={mode !== null}
	onOpenChange={(open) => {
		if (!open) close();
	}}
>
	<Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header
			><Dialog.Title>{title}</Dialog.Title><Dialog.Description
				>{roster.homeroom.name} · {mode === 'renumber'
					? 'จัดเลขที่ทั้งห้อง แม้หน้ารายชื่อจะกำลังค้นหาบางคนอยู่'
					: 'ตรวจสอบรายชื่อและข้อมูลก่อนบันทึก'}</Dialog.Description
			></Dialog.Header
		>
		<form class="space-y-4" onsubmit={save}>
			{#if mode === 'renumber'}
				<div class="grid gap-3 sm:grid-cols-2">
					<div class="space-y-1.5">
						<Label for="numbering-method">วิธีเรียงเลขที่</Label>
						<Select.Root
							type="single"
							value={method}
							onValueChange={(value) => {
								const option = numberingMethods.find((item) => item.value === value);
								if (option) {
									method = option.value;
									clearPreview();
								}
							}}
						>
							<Select.Trigger id="numbering-method" class="w-full" disabled={busy}>
								{numberingMethods.find((option) => option.value === method)?.label}
							</Select.Trigger>
							<Select.Content
								>{#each numberingMethods as option (option.value)}
									<Select.Item value={option.value}>{option.label}</Select.Item>
								{/each}</Select.Content
							>
						</Select.Root>
					</div>
					<div class="space-y-1.5">
						<Label for="numbering-start">เลขที่เริ่มต้น</Label><Input
							id="numbering-start"
							type="number"
							min="1"
							max="2147483000"
							step="1"
							bind:value={startNumber}
							oninput={clearPreview}
							disabled={busy}
						/>
					</div>
				</div>
				<Button type="button" variant="outline" onclick={loadPreview} disabled={busy || reading}
					>ดูตัวอย่างเลขที่ใหม่</Button
				>
				{#if reading}<PageSkeleton variant="table" rows={4} />{:else if preview}
					<p class="text-sm" role="status">
						เลขที่เปลี่ยน {changedCount} คน จากทั้งหมด {preview.numbers.length} คน
					</p>
					<div class="max-h-72 overflow-auto rounded-xl border">
						<Table.Root
							><Table.Header
								><Table.Row
									><Table.Head class="min-w-48">ชื่อ–นามสกุล</Table.Head><Table.Head
										>เลขที่เดิม</Table.Head
									><Table.Head>เลขที่ใหม่</Table.Head></Table.Row
								></Table.Header
							><Table.Body>
								{#each preview.numbers as input (input.placementId)}{@const student =
										preview.roster.students.find(
											(row) => row.placementId === input.placementId
										)}<Table.Row
										><Table.Cell>{student?.firstName} {student?.lastName}</Table.Cell><Table.Cell
											>{student?.classNumber ?? '—'}</Table.Cell
										><Table.Cell class="font-semibold tabular-nums">{input.classNumber}</Table.Cell
										></Table.Row
									>{/each}
							</Table.Body></Table.Root
						>
					</div>
				{/if}
			{:else if mode === 'number'}
				<p class="text-sm">
					{students[0]?.firstName}
					{students[0]?.lastName} · เลขที่เดิม {students[0]?.classNumber ?? '—'}
				</p>
				<div class="space-y-1.5">
					<Label for="student-number">เลขที่ใหม่</Label><Input
						id="student-number"
						type="number"
						min="1"
						max="2147483647"
						step="1"
						bind:value={number}
						required
						disabled={busy}
					/>
				</div>
			{:else}
				{#if mode === 'add'}
					<div class="space-y-1.5">
						<Label for="candidate-search">ค้นหานักเรียนที่ยังไม่มีห้อง</Label>
						<div class="flex gap-2">
							<Input
								id="candidate-search"
								bind:value={search}
								placeholder="ชื่อหรือรหัสนักเรียน"
								disabled={busy}
							/><Button
								type="button"
								variant="outline"
								onclick={loadCandidates}
								disabled={busy || reading}>ค้นหา</Button
							>
						</div>
					</div>
					<p class="text-xs text-muted-foreground">
						แสดงสูงสุด 100 คนต่อคำค้นหา ในระดับชั้นและแผนการเรียนเดียวกัน เลือกแล้ว {candidateIds.length}
						คน
					</p>
					{#if reading}<PageSkeleton
							variant="table"
							rows={4}
						/>{:else if candidates.length === 0}<PageState
							variant="empty"
							title="ไม่มีนักเรียนที่จัดเข้าห้องได้"
							description="ลองเปลี่ยนคำค้นหา หรือเพิ่มข้อมูลนักเรียนในหน้า นักเรียนประจำปี ก่อน"
						/>{:else}<div class="max-h-60 overflow-y-auto rounded-xl border p-3 space-y-3">
							{#each candidates as candidate (candidate.studentAcademicYearId)}<div
									class="flex items-center gap-3"
								>
									<Checkbox
										id={`candidate-${candidate.studentAcademicYearId}`}
										checked={candidateIds.includes(candidate.studentAcademicYearId)}
										onCheckedChange={(checked) =>
											toggleCandidate(candidate.studentAcademicYearId, checked)}
										disabled={busy}
									/><Label
										for={`candidate-${candidate.studentAcademicYearId}`}
										class="cursor-pointer"
										>{candidate.studentCode ?? 'ยังไม่มีรหัส'} · {candidate.name}</Label
									>
								</div>{/each}
						</div>{/if}
					<p class="text-sm text-muted-foreground">
						นักเรียนใหม่จะได้เลขที่ต่อท้าย คนเดิมคงเลขที่เดิมไว้
					</p>
				{:else}
					<div class="max-h-36 overflow-y-auto rounded-xl border bg-muted/20 p-3">
						<p class="mb-2 text-sm font-medium">นักเรียนที่เลือก {students.length} คน</p>
						{#each students as student (student.placementId)}<p class="text-sm">
								เลขที่ {student.classNumber ?? '—'} · {student.firstName}
								{student.lastName}
							</p>{/each}
					</div>
					{#if mode === 'transfer'}
						<div class="space-y-1.5">
							<Label for="target-room">ห้องปลายทาง</Label>
							<Select.Root type="single" bind:value={targetId}>
								<Select.Trigger id="target-room" class="w-full" disabled={busy || reading}>
									{reading
										? 'กำลังโหลดห้อง…'
										: (rooms.find((room) => room.id === targetId)?.name ?? 'เลือกห้องปลายทาง')}
								</Select.Trigger>
								<Select.Content
									>{#each rooms as room (room.id)}
										<Select.Item value={room.id}
											>{room.name} · {room.studentCount ?? 0} / {room.capacity} คน</Select.Item
										>
									{/each}</Select.Content
								>
							</Select.Root>
						</div>
						<p class="text-xs text-muted-foreground">
							เลือกได้เฉพาะห้องในปี ระดับชั้น และแผนการเรียนเดียวกัน
							ระบบตรวจความจุอีกครั้งก่อนบันทึก และให้เลขที่ต่อท้ายห้องใหม่
						</p>
					{:else}<p class="text-sm text-muted-foreground">
							นักเรียนจะยังมีข้อมูลประจำปีและประวัติห้องเดิม สามารถจัดเข้าห้องใหม่ภายหลังได้
							คนที่เหลือคงเลขที่เดิมไว้
						</p>{/if}
				{/if}
				<div class="space-y-1.5">
					<Label for="roster-date"
						>วันที่{mode === 'add'
							? 'เริ่มเข้าห้อง'
							: mode === 'transfer'
								? 'ย้ายห้อง'
								: 'มีผลนำออก'}</Label
					><DatePicker id="roster-date" bind:value={effectiveDate} required disabled={busy} />
				</div>
				{#if mode !== 'add'}<div class="space-y-1.5">
						<Label for="roster-reason">เหตุผล</Label><Textarea
							id="roster-reason"
							bind:value={reason}
							maxlength={500}
							required
							disabled={busy}
						/>
					</div>{/if}
			{/if}
			{#if errorMessage}<p
					role="alert"
					class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
				>
					{errorMessage}
				</p>{/if}
			<Dialog.Footer
				><Button type="button" variant="outline" onclick={close} disabled={busy}>ยกเลิก</Button
				><Button
					type="submit"
					variant={mode === 'remove' ? 'destructive' : 'default'}
					disabled={busy ||
						reading ||
						(mode === 'renumber' && !preview) ||
						(mode === 'add' && candidateIds.length === 0)}
					>{busy
						? 'กำลังบันทึก…'
						: mode === 'renumber'
							? 'บันทึกเลขที่ใหม่'
							: mode === 'remove'
								? 'ยืนยันนำออกจากห้อง'
								: 'บันทึก'}</Button
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>
