<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import {
		getRound,
		listStudentIds,
		batchUpdateStudentIds,
		sortRoomStudents,
		type StudentIdEntry
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import * as Dialog from '$lib/components/ui/dialog';
	import { toast } from 'svelte-sonner';
	import {
		ArrowLeft,
		Save,
		Wand2,
		X,
		School,
		FileSpreadsheet,
		ArrowUpDown,
		LoaderCircle,
		Upload,
		FileDown
	} from 'lucide-svelte';
	import { SvelteSet, SvelteMap } from 'svelte/reactivity';

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
	// School filter
	let schoolFilter = $state('');

	// Excel import
	let fileInput: HTMLInputElement;
	let importing = $state(false);
	let importDialogOpen = $state(false);
	type PendingMatch = { applicationId: string; studentId: string; school: string };
	let pendingMatches = $state<PendingMatch[]>([]);
	let importStats = $state<{ filled: number; ambiguous: number; notFound: number } | null>(null);

	onMount(async () => {
		try {
			const [roundRes, listRes] = await Promise.all([getRound(id), listStudentIds(id)]);
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
		return entries.filter((e) => (e.previousSchool ?? '').toLowerCase().includes(q));
	});

	// Unique school names for suggestions (sorted)
	let schoolSuggestions = $derived(() => {
		const schools = new SvelteSet<string>();
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
		const dups = new SvelteSet<string>();
		for (const [val, count] of Object.entries(counts)) {
			if (count > 1) dups.add(val);
		}
		return dups;
	});

	let assignedCount = $derived(Object.values(edits).filter((v) => v.trim()).length);
	let hasDuplicates = $derived(duplicateIds().size > 0);

	// School breakdown from pendingMatches
	let schoolBreakdown = $derived(() => {
		const counts: Record<string, number> = {};
		for (const m of pendingMatches) {
			const school = m.school || 'ไม่ระบุโรงเรียน';
			counts[school] = (counts[school] ?? 0) + 1;
		}
		return Object.entries(counts).sort((a, b) => b[1] - a[1]);
	});

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

	function autoFill() {
		const start = parseInt(startNumber, 10);
		if (isNaN(start) || start < 1) {
			toast.error('กรุณากรอกเลขเริ่มต้นที่ถูกต้อง');
			return;
		}

		// Collect ALL existing values (including non-filtered rows) to avoid collisions
		const occupied = new SvelteSet<string>();
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

	function clearAllEdits() {
		const cleared: Record<string, string> = {};
		for (const id of Object.keys(edits)) cleared[id] = '';
		edits = cleared;
	}

	function clearSchoolFilter() {
		schoolFilter = '';
	}

	async function downloadXlsx() {
		const XLSX = await import('xlsx');
		const rankColLabel = assignmentMode === 'global' ? 'อันดับรวม' : 'อันดับในสาย';
		const header = [
			'ลำดับ',
			'เลขประจำตัว',
			'เลขที่นั่งสอบ',
			'ชื่อ-สกุล',
			'เลขบัตรประจำตัวประชาชน',
			'เลขที่สมัคร',
			'สายที่สมัคร',
			'สายที่จัด',
			'โรงเรียนเดิม',
			'ห้องที่ได้',
			'เลขที่',
			rankColLabel
		];
		const rows = entries.map((e, i) => {
			return [
				i + 1,
				edits[e.applicationId]?.trim() || '',
				e.examId ?? '',
				e.fullName,
				e.nationalId ?? '',
				e.applicationNumber ?? '',
				e.originalTrackName ?? '',
				e.assignedTrackName ?? e.originalTrackName ?? '',
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

	async function downloadTemplate() {
		const XLSX = await import('xlsx');
		const ws = XLSX.utils.aoa_to_sheet([['เลขประจำตัว', 'คำนำหน้า', 'ชื่อ', 'นามสกุล']]);
		// set column widths
		ws['!cols'] = [{ wch: 14 }, { wch: 12 }, { wch: 20 }, { wch: 20 }];
		const wb = XLSX.utils.book_new();
		XLSX.utils.book_append_sheet(wb, ws, 'รายชื่อ');
		XLSX.writeFile(wb, 'template-เลขประจำตัว.xlsx');
	}

	function normalize(s: string) {
		return s.trim().replace(/\s+/g, ' ');
	}

	async function importFromExcel(file: File) {
		importing = true;
		try {
			const XLSX = await import('xlsx');
			const buf = await file.arrayBuffer();
			const wb = XLSX.read(buf, { type: 'array' });
			const ws = wb.Sheets[wb.SheetNames[0]];
			const rows = XLSX.utils.sheet_to_json<Record<string, unknown>>(ws, { defval: '' });

			if (rows.length === 0) {
				toast.error('ไม่พบข้อมูลในไฟล์');
				return;
			}

			// Detect columns
			const headers = Object.keys(rows[0]);
			const idCol = headers.find((h) => /เลข|id|รหัส/i.test(h) && !/นามสกุล|สกุล/i.test(h));
			const firstCol = headers.find(
				(h) =>
					/^ชื่อ$|^ชื่อตัว$|^firstname$/i.test(h.trim()) ||
					(h.includes('ชื่อ') && !h.includes('สกุล') && !h.includes('นาม'))
			);
			const lastCol = headers.find((h) => /สกุล/i.test(h));

			if (!idCol || !firstCol || !lastCol) {
				toast.error(
					`ไม่พบคอลัมน์ที่ต้องการ (เลขประจำตัว / ชื่อ / นามสกุล)\nพบ: ${headers.join(', ')}`
				);
				return;
			}

			// Build lookup map: "ชื่อ นามสกุล" → applicationId[]
			const lookup = new SvelteMap<string, string[]>();
			for (const e of entries) {
				const key = normalize(`${e.firstName} ${e.lastName}`);
				if (!lookup.has(key)) lookup.set(key, []);
				lookup.get(key)!.push(e.applicationId);
			}

			// Build school lookup: applicationId → previousSchool
			const schoolMap = new SvelteMap<string, string>();
			for (const e of entries) {
				schoolMap.set(e.applicationId, e.previousSchool ?? '');
			}

			const matches: PendingMatch[] = [];
			let ambiguous = 0;
			let notFound = 0;

			for (const row of rows) {
				const studentId = String(row[idCol]).trim();
				const firstName = normalize(String(row[firstCol]));
				const lastName = normalize(String(row[lastCol]));
				if (!studentId || !firstName || !lastName) continue;

				const key = normalize(`${firstName} ${lastName}`);
				const appIds = lookup.get(key);

				if (!appIds || appIds.length === 0) {
					notFound++;
				} else if (appIds.length > 1) {
					ambiguous++;
				} else {
					matches.push({
						applicationId: appIds[0],
						studentId,
						school: schoolMap.get(appIds[0]) ?? ''
					});
				}
			}

			pendingMatches = matches;
			importStats = { filled: matches.length, ambiguous, notFound };
			importDialogOpen = true;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'อ่านไฟล์ไม่สำเร็จ');
		} finally {
			importing = false;
			// reset file input
			fileInput.value = '';
		}
	}

	function confirmImport() {
		const newEdits = { ...edits };
		for (const m of pendingMatches) {
			newEdits[m.applicationId] = m.studentId;
		}
		edits = newEdits;
		importDialogOpen = false;
		toast.success(`กรอกเลขประจำตัวสำเร็จ ${pendingMatches.length} คน`);
	}
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

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
		<Card.Content class="py-3 flex items-center gap-2 overflow-x-auto flex-nowrap">
			<!-- Stats -->
			<p class="text-sm text-muted-foreground whitespace-nowrap shrink-0">
				{#if loading}
					กำลังโหลด...
				{:else}
					<span class="font-semibold text-foreground">{assignedCount}</span>/{entries.length} คน
					{#if hasDuplicates}
						<span class="text-destructive ml-1">· ซ้ำ {duplicateIds().size}</span>
					{/if}
				{/if}
			</p>

			<div class="w-px h-5 bg-border shrink-0"></div>

			<!-- School filter -->
			<div class="relative shrink-0">
				<School
					class="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground"
				/>
				<Input
					list="school-suggestions"
					bind:value={schoolFilter}
					placeholder="กรองโรงเรียน..."
					class="pl-7 pr-7 h-8 text-sm w-40"
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
					{#each schoolSuggestions() as school (school)}
						<option value={school}></option>
					{/each}
				</datalist>
			</div>

			<div class="w-px h-5 bg-border shrink-0"></div>

			<!-- Sort rooms -->
			<Button
				variant="outline"
				size="sm"
				class="gap-1.5 h-8 shrink-0"
				disabled={sorting || loading || entries.length === 0}
				onclick={handleSortRooms}
				title="เรียงนักเรียนในแต่ละห้องใหม่: ชายก่อน ตามด้วยหญิง แต่ละกลุ่มเรียงตามชื่อ ก-ฮ"
			>
				{#if sorting}
					<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
				{:else}
					<ArrowUpDown class="w-3.5 h-3.5" />
				{/if}
				จัดเรียงในห้อง
			</Button>

			<!-- Excel import -->
			<Button
				variant="outline"
				size="sm"
				class="gap-1.5 h-8 shrink-0"
				onclick={downloadTemplate}
				title="โหลดไฟล์ template สำหรับกรอกข้อมูล"
			>
				<FileDown class="w-3.5 h-3.5" /> Template
			</Button>
			<Button
				variant="outline"
				size="sm"
				class="gap-1.5 h-8 shrink-0"
				disabled={importing || loading || entries.length === 0}
				onclick={() => fileInput.click()}
				title="นำเข้าเลขประจำตัวจากไฟล์ Excel"
			>
				{#if importing}
					<LoaderCircle class="w-3.5 h-3.5 animate-spin" />
				{:else}
					<Upload class="w-3.5 h-3.5" />
				{/if}
				นำเข้า Excel
			</Button>
			<input
				bind:this={fileInput}
				type="file"
				accept=".xlsx,.xls,.csv"
				class="hidden"
				onchange={(e) => {
					const file = (e.target as HTMLInputElement).files?.[0];
					if (file) importFromExcel(file);
				}}
			/>

			<div class="w-px h-5 bg-border shrink-0"></div>

			<!-- Auto-fill -->
			<Input
				type="number"
				min="1"
				bind:value={startNumber}
				title="เลขเริ่มต้น"
				class="w-20 h-8 text-sm shrink-0"
			/>
			<Button
				variant="outline"
				size="sm"
				class="gap-1.5 h-8 shrink-0"
				onclick={autoFill}
				title={schoolFilter.trim() ? 'Auto-fill เฉพาะที่กรอง' : 'Auto-fill ช่องว่าง'}
			>
				<Wand2 class="w-3.5 h-3.5" />
				Auto-fill
			</Button>

			<div class="w-px h-5 bg-border shrink-0"></div>

			<!-- Save + Download + Clear -->
			<Button size="sm" class="gap-1.5 h-8 shrink-0" onclick={saveAll} disabled={saving || loading}>
				{#if saving}
					<span class="animate-spin">⏳</span>
				{:else}
					<Save class="w-3.5 h-3.5" />
				{/if}
				บันทึก
			</Button>
			<Button
				variant="outline"
				size="sm"
				class="gap-1.5 h-8 shrink-0"
				onclick={downloadXlsx}
				disabled={loading || entries.length === 0}
				title="ดาวน์โหลด XLSX"
			>
				<FileSpreadsheet class="w-3.5 h-3.5" />
			</Button>
			<Button
				variant="ghost"
				size="sm"
				class="gap-1.5 h-8 shrink-0 text-destructive hover:text-destructive"
				onclick={clearAllEdits}
				disabled={loading || assignedCount === 0}
				title="ล้างเลขประจำตัวทั้งหมด"
			>
				<X class="w-3.5 h-3.5" /> ล้าง
			</Button>
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
					<Table.Head class="w-20 text-center"
						>{assignmentMode === 'global' ? 'อันดับรวม' : 'อันดับในสาย'}</Table.Head
					>
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
							{entries.length === 0
								? 'ไม่มีนักเรียนที่ผ่านการคัดเลือก'
								: 'ไม่พบนักเรียนจากโรงเรียนที่ค้นหา'}
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
									<span class="text-muted-foreground line-through text-xs"
										>{entry.originalTrackName ?? '-'}</span
									>
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
										edits = {
											...edits,
											[entry.applicationId]: (e.target as HTMLInputElement).value
										};
									}}
									class="h-7 text-sm {isDup
										? 'border-destructive focus-visible:ring-destructive'
										: ''}"
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

<!-- Import Confirmation Dialog -->
<Dialog.Root bind:open={importDialogOpen}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>สรุปผลการนำเข้า</Dialog.Title>
			<Dialog.Description>ตรวจสอบข้อมูลก่อนยืนยันการกรอกเลขประจำตัว</Dialog.Description>
		</Dialog.Header>

		{#if importStats}
			<div class="space-y-4 py-2">
				<!-- Summary stats -->
				<div class="space-y-1.5">
					<div class="flex items-center gap-2 text-sm">
						<span
							class="w-5 h-5 rounded-full bg-green-100 text-green-700 flex items-center justify-center text-xs font-bold"
							>✓</span
						>
						<span>จะกรอกได้</span>
						<span class="ml-auto font-semibold">{importStats.filled} คน</span>
					</div>
					{#if importStats.ambiguous > 0}
						<div class="flex items-center gap-2 text-sm text-amber-600">
							<span
								class="w-5 h-5 rounded-full bg-amber-100 flex items-center justify-center text-xs font-bold"
								>!</span
							>
							<span>ชื่อซ้ำในระบบ (ข้าม)</span>
							<span class="ml-auto font-semibold">{importStats.ambiguous} คน</span>
						</div>
					{/if}
					{#if importStats.notFound > 0}
						<div class="flex items-center gap-2 text-sm text-muted-foreground">
							<span
								class="w-5 h-5 rounded-full bg-muted flex items-center justify-center text-xs font-bold"
								>–</span
							>
							<span>ไม่พบในรายชื่อผู้สมัคร</span>
							<span class="ml-auto font-semibold">{importStats.notFound} คน</span>
						</div>
					{/if}
				</div>

				<!-- School breakdown -->
				{#if schoolBreakdown().length > 0 && importStats.filled > 0}
					<div>
						<p class="text-xs text-muted-foreground mb-1.5">รายละเอียดตามโรงเรียน:</p>
						<div class="max-h-48 overflow-y-auto space-y-1 rounded border p-2">
							{#each schoolBreakdown() as [school, count] (school)}
								<div class="flex items-center justify-between text-sm">
									<span class="truncate text-muted-foreground">{school}</span>
									<span class="ml-2 font-medium shrink-0">{count} คน</span>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/if}

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (importDialogOpen = false)}>ยกเลิก</Button>
			<Button onclick={confirmImport} disabled={!importStats || importStats.filled === 0}>
				ยืนยันการกรอก {importStats?.filled ?? 0} คน
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
