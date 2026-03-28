<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { getRound, listStudentIds, batchUpdateStudentIds, sortRoomStudents, autoAssignStudentIds, type StudentIdEntry } from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Hash, Save, Wand2, X, AlertTriangle, School, FileSpreadsheet, ArrowUpDown, LoaderCircle } from 'lucide-svelte';

	let { data, params }: PageProps = $props();
	let id = $derived(params.id);

	let roundName = $state('');
	let assignmentMode = $state<'per_track' | 'global' | undefined>(undefined);
	let entries: StudentIdEntry[] = $state([]);
	let loading = $state(true);
	let saving = $state(false);

	// Local edits: applicationId -> studentId string
	let edits = $state<Record<string, string>>({});

	let startNumber = $state('1');
	let sorting = $state(false);
	let autoAssigning = $state(false);

	// School filter
	let schoolFilter = $state('');

	onMount(async () => {
		try {
			const [roundRes, listRes] = await Promise.all([
				getRound(id),
				listStudentIds(id)
			]);
			roundName = roundRes.name ?? '';
			assignmentMode = roundRes.selectionSettings?.assignmentMode;
			entries = listRes.data;
			// Seed edits with existing assigned values
			const init: Record<string, string> = {};
			for (const e of listRes.data) {
				init[e.applicationId] = e.assignedStudentId ?? '';
			}
			edits = init;
		} catch {
			toast.error('โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	});

	// Filtered entries based on school search
	let filteredEntries = $derived(() => {
		const q = schoolFilter.trim().toLowerCase();
		if (!q) return entries;
		return entries.filter((e) =>
			(e.previousSchool ?? '').toLowerCase().includes(q)
		);
	});

	// Unique school names for suggestions (sorted)
	let schoolSuggestions = $derived(() => {
		const schools = new Set<string>();
		for (const e of entries) {
			if (e.previousSchool?.trim()) schools.add(e.previousSchool.trim());
		}
		return [...schools].sort();
	});

	// Set of IDs that appear more than once (duplicates within batch — across ALL entries, not just filtered)
	let duplicateIds = $derived(() => {
		const counts: Record<string, number> = {};
		for (const val of Object.values(edits)) {
			if (val.trim()) {
				counts[val.trim()] = (counts[val.trim()] ?? 0) + 1;
			}
		}
		const dups = new Set<string>();
		for (const [val, count] of Object.entries(counts)) {
			if (count > 1) dups.add(val);
		}
		return dups;
	});

	let assignedCount = $derived(Object.values(edits).filter((v) => v.trim()).length);
	let hasDuplicates = $derived(duplicateIds().size > 0);

	async function handleSortRooms() {
		sorting = true;
		try {
			const res = await sortRoomStudents(id);
			toast.success(`จัดเรียงสำเร็จ ${res.updated} คน`);
			// reload list เพื่อให้ rankInRoom อัปเดต
			const listRes = await listStudentIds(id);
			entries = listRes.data;
			const newEdits: Record<string, string> = {};
			for (const e of listRes.data) {
				newEdits[e.applicationId] = edits[e.applicationId] ?? e.assignedStudentId ?? '';
			}
			edits = newEdits;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'จัดเรียงไม่สำเร็จ');
		} finally {
			sorting = false;
		}
	}

	async function handleAutoAssign() {
		const start = parseInt(startNumber, 10);
		if (isNaN(start) || start < 1) {
			toast.error('กรุณากรอกเลขเริ่มต้นที่ถูกต้อง');
			return;
		}
		autoAssigning = true;
		try {
			const res = await autoAssignStudentIds(id, start);
			toast.success(`กำหนดเลขสำเร็จ ${res.assigned} คน`);
			// reload
			const listRes = await listStudentIds(id);
			entries = listRes.data;
			const newEdits: Record<string, string> = {};
			for (const e of listRes.data) {
				newEdits[e.applicationId] = e.assignedStudentId ?? '';
			}
			edits = newEdits;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'กำหนดเลขไม่สำเร็จ');
		} finally {
			autoAssigning = false;
		}
	}

	function autoFill() {
		const start = parseInt(startNumber, 10);
		if (isNaN(start) || start < 1) {
			toast.error('กรุณากรอกเลขเริ่มต้นที่ถูกต้อง');
			return;
		}

		// Collect ALL existing values (including non-filtered rows) to avoid collisions
		const occupied = new Set<string>();
		for (const e of entries) {
			const val = edits[e.applicationId]?.trim();
			if (val) occupied.add(val);
		}

		let next = start;
		const newEdits = { ...edits };
		// Auto-fill only the filtered rows
		for (const e of filteredEntries()) {
			const current = newEdits[e.applicationId]?.trim();
			if (current) continue;

			while (occupied.has(String(next))) {
				next++;
			}
			const val = String(next);
			newEdits[e.applicationId] = val;
			occupied.add(val);
			next++;
		}
		edits = newEdits;
	}

	async function saveAll() {
		if (hasDuplicates) {
			toast.error('มีเลขประจำตัวซ้ำกัน กรุณาแก้ไขก่อนบันทึก');
			return;
		}
		saving = true;
		try {
			const updates = entries.map((e) => ({
				applicationId: e.applicationId,
				studentId: edits[e.applicationId]?.trim() || null
			}));
			const res = await batchUpdateStudentIds(id, updates);
			toast.success(`บันทึกสำเร็จ ${res.updated} รายการ`);
		} catch {
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	function clearEntry(appId: string) {
		edits = { ...edits, [appId]: '' };
	}

	function clearSchoolFilter() {
		schoolFilter = '';
	}

	async function downloadXlsx() {
		const XLSX = await import('xlsx');
		const rankColLabel = assignmentMode === 'global' ? 'อันดับรวม' : 'อันดับในสาย';
		const header = ['ลำดับ', 'เลขประจำตัว', 'ชื่อ-สกุล', 'เลขที่สมัคร', 'สายการเรียน', 'โรงเรียนเดิม', 'ห้องที่ได้', 'เลขที่', rankColLabel];
		const rows = entries.map((e, i) => {
			const trackName = e.assignedTrackName
				? `${e.originalTrackName ?? ''} → ${e.assignedTrackName}`
				: (e.originalTrackName ?? '');
			return [
				i + 1,
				edits[e.applicationId]?.trim() || '',
				e.fullName,
				e.applicationNumber ?? '',
				trackName,
				e.previousSchool ?? '',
				e.roomName ?? '',
				e.rankInRoom ?? '',
				e.rankInTrack ?? ''
			];
		});
		const ws = XLSX.utils.aoa_to_sheet([header, ...rows]);
		const wb = XLSX.utils.book_new();
		XLSX.utils.book_append_sheet(wb, ws, 'เลขประจำตัว');
		XLSX.writeFile(wb, `เลขประจำตัว-${roundName || id}.xlsx`);
	}
</script>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-3">
		<Button href="/staff/academic/admission/{id}" variant="ghost" size="sm" class="gap-1.5">
			<ArrowLeft class="w-4 h-4" /> กลับ
		</Button>
		<div>
			<h1 class="text-xl font-semibold">กำหนดเลขประจำตัวนักเรียน</h1>
			{#if roundName}
				<p class="text-sm text-muted-foreground">{roundName}</p>
			{/if}
		</div>
	</div>

	<!-- Controls -->
	<Card.Root>
		<Card.Content class="pt-4 flex flex-wrap gap-4 items-end">
			<!-- Stats -->
			<div class="flex-1 min-w-[180px]">
				{#if loading}
					<p class="text-sm text-muted-foreground">กำลังโหลด...</p>
				{:else}
					<p class="text-sm text-muted-foreground">
						กำหนดแล้ว
						<span class="font-semibold text-foreground">{assignedCount}</span>
						/ {entries.length} คน
						{#if schoolFilter.trim()}
							<span class="ml-1">(แสดง {filteredEntries().length} คน)</span>
						{/if}
					</p>
					{#if hasDuplicates}
						<p class="text-sm text-destructive flex items-center gap-1 mt-0.5">
							<AlertTriangle class="w-3.5 h-3.5" />
							มีเลขซ้ำกัน {duplicateIds().size} เลข
						</p>
					{/if}
				{/if}
			</div>

			<!-- School filter -->
			<div class="space-y-1">
				<Label class="text-xs">กรองตามโรงเรียนเดิม</Label>
				<div class="relative">
					<School class="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
					<Input
						list="school-suggestions"
						bind:value={schoolFilter}
						placeholder="พิมพ์ชื่อโรงเรียน..."
						class="pl-7 pr-7 h-8 text-sm w-52"
					/>
					{#if schoolFilter}
						<button
							type="button"
							onclick={clearSchoolFilter}
							class="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
						>
							<X class="w-3 h-3" />
						</button>
					{/if}
					<datalist id="school-suggestions">
						{#each schoolSuggestions() as school}
							<option value={school}></option>
						{/each}
					</datalist>
				</div>
			</div>

			<!-- Sort rooms -->
			<Button
				variant="outline"
				size="sm"
				class="gap-1.5 h-8"
				disabled={sorting || loading || entries.length === 0}
				onclick={handleSortRooms}
				title="เรียงนักเรียนในแต่ละห้องใหม่: ชายก่อน ตามด้วยหญิง แต่ละกลุ่มเรียงตามชื่อ ก-ฮ"
			>
				{#if sorting}
					<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
				{:else}
					<ArrowUpDown class="w-3.5 h-3.5" />
				{/if}
				จัดเรียงในห้อง (ชาย→หญิง ก-ฮ)
			</Button>

			<!-- Auto-fill + Auto-assign -->
			<div class="flex items-end gap-2">
				<div class="space-y-1">
					<Label class="text-xs">เลขเริ่มต้น</Label>
					<Input
						type="number"
						min="1"
						bind:value={startNumber}
						class="w-24 h-8 text-sm"
					/>
				</div>
				<Button variant="outline" size="sm" class="gap-1.5 h-8" onclick={autoFill}>
					<Wand2 class="w-3.5 h-3.5" />
					{schoolFilter.trim() ? 'Auto-fill ที่กรอง' : 'Auto-fill ช่องว่าง'}
				</Button>
				<Button variant="secondary" size="sm" class="gap-1.5 h-8"
					disabled={autoAssigning || loading || entries.length === 0}
					onclick={handleAutoAssign}
					title="กำหนดเลขประจำตัวอัตโนมัติตามลำดับห้อง และบันทึกลง DB ทันที (เฉพาะที่ยังว่าง)"
				>
					{#if autoAssigning}
						<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
					{:else}
						<Hash class="w-3.5 h-3.5" />
					{/if}
					กำหนดเลขอัตโนมัติ
				</Button>
			</div>

			<!-- Save + Download -->
			<div class="flex gap-2">
				<Button
					size="sm"
					class="gap-1.5 h-8"
					onclick={saveAll}
					disabled={saving || loading}
				>
					{#if saving}
						<span class="animate-spin">⏳</span>
					{:else}
						<Save class="w-3.5 h-3.5" />
					{/if}
					บันทึกทั้งหมด
				</Button>
				<Button
					variant="outline"
					size="sm"
					class="gap-1.5 h-8"
					onclick={downloadXlsx}
					disabled={loading || entries.length === 0}
				>
					<FileSpreadsheet class="w-3.5 h-3.5" /> ดาวน์โหลด XLSX
				</Button>
			</div>
		</Card.Content>
	</Card.Root>

	<!-- Table -->
	<Card.Root>
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head class="w-10">#</Table.Head>
					<Table.Head>ชื่อ-สกุล</Table.Head>
					<Table.Head>เลขที่สมัคร</Table.Head>
					<Table.Head>สายการเรียน</Table.Head>
					<Table.Head>โรงเรียนเดิม</Table.Head>
					<Table.Head>ห้องที่ได้</Table.Head>
					<Table.Head class="w-16 text-center">เลขที่</Table.Head>
					<Table.Head class="w-20 text-center">{assignmentMode === 'global' ? 'อันดับรวม' : 'อันดับในสาย'}</Table.Head>
					<Table.Head class="w-44">เลขประจำตัว</Table.Head>
					<Table.Head class="w-8"></Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#if loading}
					<Table.Row>
						<Table.Cell colspan={10} class="text-center text-muted-foreground py-8">
							กำลังโหลด...
						</Table.Cell>
					</Table.Row>
				{:else if filteredEntries().length === 0}
					<Table.Row>
						<Table.Cell colspan={10} class="text-center text-muted-foreground py-8">
							{entries.length === 0 ? 'ไม่มีนักเรียนที่ผ่านการคัดเลือก' : 'ไม่พบนักเรียนจากโรงเรียนที่ค้นหา'}
						</Table.Cell>
					</Table.Row>
				{:else}
					{#each filteredEntries() as entry, i (entry.applicationId)}
						{@const val = edits[entry.applicationId] ?? ''}
						{@const isDup = val.trim() ? duplicateIds().has(val.trim()) : false}
						<Table.Row class={isDup ? 'bg-destructive/5' : ''}>
							<Table.Cell class="text-muted-foreground text-sm">{i + 1}</Table.Cell>
							<Table.Cell class="font-medium">{entry.fullName}</Table.Cell>
							<Table.Cell class="text-sm text-muted-foreground">
								{entry.applicationNumber ?? '-'}
							</Table.Cell>
							<Table.Cell class="text-sm">
								{#if entry.assignedTrackName}
									<span class="text-muted-foreground line-through text-xs">{entry.originalTrackName ?? '-'}</span>
									<br />
									<span class="font-medium">{entry.assignedTrackName}</span>
								{:else}
									{entry.originalTrackName ?? '-'}
								{/if}
							</Table.Cell>
							<Table.Cell class="text-sm text-muted-foreground max-w-[160px] truncate">
								{entry.previousSchool ?? '-'}
							</Table.Cell>
							<Table.Cell class="text-sm">
								{#if entry.roomName}
									<Badge variant="outline" class="text-xs">{entry.roomName}</Badge>
								{:else}
									<span class="text-muted-foreground">-</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-center text-sm">
								{entry.rankInRoom ?? '-'}
							</Table.Cell>
							<Table.Cell class="text-center text-sm">
								{entry.rankInTrack ?? '-'}
							</Table.Cell>
							<Table.Cell>
								<Input
									value={val}
									oninput={(e) => {
										edits = { ...edits, [entry.applicationId]: (e.target as HTMLInputElement).value };
									}}
									class="h-7 text-sm {isDup ? 'border-destructive focus-visible:ring-destructive' : ''}"
									placeholder="กรอกเลข..."
								/>
							</Table.Cell>
							<Table.Cell>
								{#if val}
									<Button
										variant="ghost"
										size="icon"
										class="w-6 h-6"
										onclick={() => clearEntry(entry.applicationId)}
									>
										<X class="w-3 h-3" />
									</Button>
								{/if}
							</Table.Cell>
						</Table.Row>
					{/each}
				{/if}
			</Table.Body>
		</Table.Root>
	</Card.Root>
</div>
