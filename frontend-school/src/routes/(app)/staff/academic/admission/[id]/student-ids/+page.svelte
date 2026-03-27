<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { getRound, listStudentIds, batchUpdateStudentIds, type StudentIdEntry } from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Table from '$lib/components/ui/table';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Hash, Save, Wand2, X, AlertTriangle } from 'lucide-svelte';

	let { data, params }: PageProps = $props();
	let id = $derived(params.id);

	let roundName = $state('');
	let entries: StudentIdEntry[] = $state([]);
	let loading = $state(true);
	let saving = $state(false);

	// Local edits: applicationId -> studentId string
	let edits = $state<Record<string, string>>({});

	let startNumber = $state('1');

	onMount(async () => {
		try {
			const [roundRes, listRes] = await Promise.all([
				getRound(id),
				listStudentIds(id)
			]);
			roundName = roundRes.name ?? '';
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

	// Set of IDs that appear more than once (duplicates within batch)
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

	function autoFill() {
		const start = parseInt(startNumber, 10);
		if (isNaN(start) || start < 1) {
			toast.error('กรุณากรอกเลขเริ่มต้นที่ถูกต้อง');
			return;
		}

		// Collect existing values that should not be overwritten
		const occupied = new Set<string>();
		for (const e of entries) {
			const val = edits[e.applicationId]?.trim();
			if (val) occupied.add(val);
		}

		let next = start;
		const newEdits = { ...edits };
		for (const e of entries) {
			const current = newEdits[e.applicationId]?.trim();
			if (current) continue; // already has a value, skip

			// Find next number not already occupied
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
					</p>
					{#if hasDuplicates}
						<p class="text-sm text-destructive flex items-center gap-1 mt-0.5">
							<AlertTriangle class="w-3.5 h-3.5" />
							มีเลขซ้ำกัน {duplicateIds().size} เลข
						</p>
					{/if}
				{/if}
			</div>

			<!-- Auto-fill -->
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
					<Wand2 class="w-3.5 h-3.5" /> Auto-fill ช่องว่าง
				</Button>
			</div>

			<!-- Save -->
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
					<Table.Head>ห้องที่ได้</Table.Head>
					<Table.Head class="w-16 text-center">อันดับ</Table.Head>
					<Table.Head class="w-44">เลขประจำตัว</Table.Head>
					<Table.Head class="w-8"></Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#if loading}
					<Table.Row>
						<Table.Cell colspan={7} class="text-center text-muted-foreground py-8">
							กำลังโหลด...
						</Table.Cell>
					</Table.Row>
				{:else if entries.length === 0}
					<Table.Row>
						<Table.Cell colspan={7} class="text-center text-muted-foreground py-8">
							ไม่มีนักเรียนที่ผ่านการคัดเลือก
						</Table.Cell>
					</Table.Row>
				{:else}
					{#each entries as entry, i (entry.applicationId)}
						{@const val = edits[entry.applicationId] ?? ''}
						{@const isDup = val.trim() && duplicateIds().has(val.trim())}
						<Table.Row class={isDup ? 'bg-destructive/5' : ''}>
							<Table.Cell class="text-muted-foreground text-sm">{i + 1}</Table.Cell>
							<Table.Cell class="font-medium">{entry.fullName}</Table.Cell>
							<Table.Cell class="text-sm text-muted-foreground">
								{entry.applicationNumber ?? '-'}
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
