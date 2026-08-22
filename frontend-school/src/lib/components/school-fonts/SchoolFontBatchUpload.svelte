<script lang="ts">
	import {
		deleteSchoolFontTemporaryFile,
		uploadSchoolFontFile,
		type FileMetadata,
		type SchoolFontUploadContext
	} from '$lib/api/files';
	import type {
		AttachSchoolFontBatchRequest,
		InspectSchoolFontUploadsRequest,
		SchoolFontListResponse,
		SchoolFontSummary,
		SchoolFontUploadInspection,
		SchoolFontUploadInspectionFile,
		SchoolFontUploadStatus
	} from '$lib/api/school-fonts';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { AlertTriangle, CheckCircle2, FileType2, RefreshCw, Trash2, Upload } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	const MAX_FONT_BATCH_FILES = 40;

	type UploadState = 'queued' | 'uploading' | 'uploaded' | 'upload_failed' | 'ready' | 'rejected';

	type UploadRow = {
		key: string;
		file: File;
		status: UploadState;
		metadata?: FileMetadata;
		inspection?: SchoolFontUploadInspectionFile;
		error?: string;
		cleaning?: boolean;
	};

	let {
		context,
		inspectUploads,
		attachBatch,
		onattached,
		onpendingchange = () => {}
	}: {
		context: SchoolFontUploadContext;
		inspectUploads: (
			payload: InspectSchoolFontUploadsRequest
		) => Promise<SchoolFontUploadInspection>;
		attachBatch: (payload: AttachSchoolFontBatchRequest) => Promise<SchoolFontListResponse>;
		onattached: (items: SchoolFontSummary[]) => void;
		onpendingchange?: (pending: boolean) => void;
	} = $props();

	let rows = $state.raw<UploadRow[]>([]);
	let rightsConfirmed = $state(false);
	let batchError = $state<string | null>(null);
	let uploading = $state(false);
	let inspecting = $state(false);
	let attaching = $state(false);
	let inputKey = $state(0);
	let reportedPending = false;

	const hasRows = $derived(rows.length > 0);
	const allRowsReady = $derived(
		rows.length > 0 && rows.every((row) => row.status === 'ready' && row.metadata)
	);
	const isBusy = $derived(uploading || inspecting || attaching);

	const inspectionLabels: Record<SchoolFontUploadStatus, string> = {
		ready: 'พร้อมเพิ่มเข้าคลัง',
		duplicate_selection: 'variant ซ้ำในชุดนี้',
		duplicate_existing: 'มี variant นี้ในคลังแล้ว',
		unsupported_variable: 'ไม่รองรับ variable font',
		unsupported_weight: 'น้ำหนักฟอนต์นี้ยังไม่รองรับ',
		missing_family: 'อ่านชื่อ family ไม่ได้',
		invalid_display_name: 'ชื่อไฟล์ไม่ถูกต้อง',
		unavailable: 'ไฟล์ไม่พร้อมหรือไม่ใช่ฟอนต์'
	};

	function asMessage(error: unknown, fallback: string): string {
		return error instanceof Error ? error.message : fallback;
	}

	function reportPending(): void {
		const pending = rows.some((row) => row.status === 'uploading' || row.metadata !== undefined);
		if (pending === reportedPending) return;
		reportedPending = pending;
		onpendingchange(pending);
	}

	function replaceRow(key: string, patch: Partial<UploadRow>): void {
		rows = rows.map((row) => (row.key === key ? { ...row, ...patch } : row));
		reportPending();
	}

	function resetBatch(): void {
		rows = [];
		rightsConfirmed = false;
		batchError = null;
		inputKey += 1;
		reportPending();
	}

	function selectFonts(event: Event): void {
		const input = event.currentTarget as HTMLInputElement;
		const selected = Array.from(input.files ?? []);
		batchError = null;
		if (selected.length === 0) return;
		if (selected.length > MAX_FONT_BATCH_FILES) {
			batchError = `เลือกไฟล์ฟอนต์ได้ครั้งละไม่เกิน ${MAX_FONT_BATCH_FILES} ไฟล์`;
			inputKey += 1;
			return;
		}
		rows = selected.map((file) => ({
			key: crypto.randomUUID(),
			file,
			status: 'queued'
		}));
		rightsConfirmed = false;
		reportPending();
	}

	async function inspectUploadedRows(): Promise<void> {
		const uploadedRows = rows.filter((row) => row.metadata);
		if (uploadedRows.length === 0 || inspecting) return;
		inspecting = true;
		batchError = null;
		try {
			const result = await inspectUploads({
				fileIds: uploadedRows.map((row) => row.metadata!.id)
			});
			const byFileId = new Map(result.files.map((file) => [file.fileId, file]));
			rows = rows.map((row) => {
				if (!row.metadata) return row;
				const inspection = byFileId.get(row.metadata.id);
				if (!inspection) {
					return {
						...row,
						status: 'rejected',
						error: 'ผลตรวจไฟล์ไม่ครบ กรุณาลองตรวจอีกครั้ง'
					};
				}
				return {
					...row,
					inspection,
					status: inspection.status === 'ready' ? 'ready' : 'rejected',
					error: undefined
				};
			});
		} catch (error) {
			batchError = asMessage(error, 'ตรวจข้อมูลภายในไฟล์ฟอนต์ไม่สำเร็จ');
			rows = rows.map((row) =>
				row.metadata ? { ...row, status: 'uploaded', error: batchError ?? undefined } : row
			);
		} finally {
			inspecting = false;
			reportPending();
		}
	}

	async function uploadSelectedRows(onlyKey?: string): Promise<void> {
		if (isBusy) return;
		uploading = true;
		batchError = null;
		const selectedRows = rows.filter(
			(row) =>
				(!onlyKey || row.key === onlyKey) &&
				!row.metadata &&
				(row.status === 'queued' || row.status === 'upload_failed')
		);
		for (const row of selectedRows) {
			replaceRow(row.key, { status: 'uploading', error: undefined });
			try {
				const metadata = await uploadSchoolFontFile(row.file, context);
				replaceRow(row.key, { metadata, status: 'uploaded', error: undefined });
			} catch (error) {
				replaceRow(row.key, {
					status: 'upload_failed',
					error: asMessage(error, 'อัปโหลดไฟล์ฟอนต์ไม่สำเร็จ')
				});
			}
		}
		uploading = false;
		await inspectUploadedRows();
	}

	async function retryRow(key: string): Promise<void> {
		const row = rows.find((candidate) => candidate.key === key);
		if (!row) return;
		if (row.metadata) {
			await inspectUploadedRows();
			return;
		}
		await uploadSelectedRows(key);
	}

	function removeLocalRow(key: string): void {
		const row = rows.find((candidate) => candidate.key === key);
		if (!row || row.metadata || row.status === 'uploading') return;
		rows = rows.filter((candidate) => candidate.key !== key);
		if (rows.length === 0) resetBatch();
		else reportPending();
	}

	async function cleanupTemporary(key: string): Promise<void> {
		const row = rows.find((candidate) => candidate.key === key);
		if (!row?.metadata || row.cleaning) return;
		replaceRow(key, { cleaning: true, error: undefined });
		try {
			await deleteSchoolFontTemporaryFile(row.metadata.id, context);
			rows = rows.filter((candidate) => candidate.key !== key);
			batchError = null;
			if (rows.length === 0) resetBatch();
			else reportPending();
		} catch (error) {
			replaceRow(key, {
				cleaning: false,
				error: asMessage(error, 'ลบไฟล์ชั่วคราวไม่สำเร็จ')
			});
		}
	}

	async function attachReviewedBatch(): Promise<void> {
		if (!allRowsReady || !rightsConfirmed || attaching) return;
		const fileIds = rows.map((row) => row.metadata!.id);
		attaching = true;
		batchError = null;
		try {
			const result = await attachBatch({ fileIds, rightsConfirmed: true });
			onattached(result.items);
			resetBatch();
			toast.success(`เพิ่มฟอนต์ ${fileIds.length} ไฟล์เข้าคลังแล้ว`);
		} catch (error) {
			batchError = asMessage(error, 'เพิ่มชุดฟอนต์เข้าคลังไม่สำเร็จ');
		} finally {
			attaching = false;
		}
	}

	function localStatus(row: UploadRow): string {
		if (row.inspection) return inspectionLabels[row.inspection.status];
		switch (row.status) {
			case 'queued':
				return 'รออัปโหลด';
			case 'uploading':
				return 'กำลังอัปโหลด';
			case 'uploaded':
				return 'รอตรวจข้อมูลฟอนต์';
			case 'upload_failed':
				return 'อัปโหลดไม่สำเร็จ';
			case 'ready':
				return 'พร้อมเพิ่มเข้าคลัง';
			case 'rejected':
				return 'ยังเพิ่มไม่ได้';
		}
	}
</script>

<section class="space-y-4" aria-labelledby="school-font-upload-title">
	<div class="flex items-start gap-3">
		<span class="grid size-10 shrink-0 place-items-center rounded-xl bg-violet-100 text-violet-700">
			<FileType2 class="size-5" />
		</span>
		<div class="min-w-0">
			<h3 id="school-font-upload-title" class="font-medium">เพิ่มฟอนต์เข้าคลัง</h3>
			<p class="mt-0.5 text-xs leading-relaxed text-muted-foreground">
				เลือก TTF หรือ OTF แบบ static ได้ไม่เกิน {MAX_FONT_BATCH_FILES} ไฟล์ ระบบจะอ่าน family น้ำหนัก
				และตัวเอียงให้ตรวจสอบก่อนเพิ่มพร้อมกัน
			</p>
		</div>
	</div>

	<div class="space-y-2">
		<Label for="school-font-batch-input">ไฟล์ฟอนต์</Label>
		{#key inputKey}
			<Input
				id="school-font-batch-input"
				type="file"
				accept=".ttf,.otf"
				multiple
				onchange={selectFonts}
				disabled={isBusy || hasRows}
			/>
		{/key}
	</div>

	{#if batchError}
		<div
			class="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
			role="alert"
		>
			<AlertTriangle class="mt-0.5 size-4 shrink-0" />
			<p>{batchError}</p>
		</div>
	{/if}

	{#if rows.length > 0}
		<div class="divide-y overflow-hidden rounded-xl border bg-background">
			{#each rows as row (row.key)}
				<div class="grid gap-3 p-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
					<div class="min-w-0">
						<div class="flex flex-wrap items-center gap-2">
							<p class="truncate text-sm font-medium">{row.file.name}</p>
							<Badge variant={row.status === 'ready' ? 'default' : 'secondary'}>
								{localStatus(row)}
							</Badge>
						</div>
						{#if row.inspection?.fontFamily}
							<p class="mt-1 text-xs text-muted-foreground">
								{row.inspection.fontFamily} · {row.inspection.fontWeight ?? '—'} ·
								{row.inspection.fontStyle === 'italic' ? 'ตัวเอียง' : 'ตัวตรง'}
							</p>
						{/if}
						{#if row.error}
							<p class="mt-1 text-xs text-destructive" role="alert">{row.error}</p>
						{/if}
					</div>
					<div class="flex flex-wrap justify-end gap-1.5">
						{#if row.status === 'upload_failed' || row.status === 'uploaded'}
							<Button
								size="sm"
								variant="outline"
								onclick={() => retryRow(row.key)}
								disabled={isBusy}
								aria-label={`ลองอัปโหลด ${row.file.name} อีกครั้ง`}
							>
								<RefreshCw class="size-4" /> ลองอีกครั้ง
							</Button>
						{/if}
						{#if row.metadata}
							<Button
								size="icon-sm"
								variant="ghost"
								onclick={() => cleanupTemporary(row.key)}
								disabled={isBusy || row.cleaning}
								aria-label={`ลบไฟล์ชั่วคราว ${row.file.name}`}
							>
								{#if row.cleaning}
									<RefreshCw class="size-4 animate-spin" />
								{:else}
									<Trash2 class="size-4" />
								{/if}
							</Button>
						{:else if row.status !== 'uploading'}
							<Button
								size="icon-sm"
								variant="ghost"
								onclick={() => removeLocalRow(row.key)}
								aria-label={`เอา ${row.file.name} ออกจากรายการ`}
							>
								<Trash2 class="size-4" />
							</Button>
						{/if}
					</div>
				</div>
			{/each}
		</div>

		{#if allRowsReady}
			<div class="flex items-start gap-3 rounded-xl border border-violet-200 bg-violet-50/70 p-3">
				<Checkbox id="school-font-rights" bind:checked={rightsConfirmed} disabled={attaching} />
				<div class="space-y-1">
					<Label for="school-font-rights" class="leading-snug">
						ยืนยันว่ามีสิทธิ์ใช้ฟอนต์เหล่านี้ในงานของโรงเรียน
					</Label>
					<p class="text-xs leading-relaxed text-muted-foreground">
						ยืนยันครั้งเดียวสำหรับชุดนี้ ฟอนต์ที่เพิ่มแล้วจะใช้ร่วมกันได้ทั้งโรงเรียน
					</p>
				</div>
			</div>
		{/if}

		<div class="flex flex-wrap items-center gap-2">
			{#if rows.some((row) => row.status === 'queued' || row.status === 'upload_failed')}
				<LoadingButton loading={uploading} onclick={() => uploadSelectedRows()}>
					<Upload class="size-4" /> อัปโหลดและตรวจฟอนต์
				</LoadingButton>
			{/if}
			{#if allRowsReady}
				<LoadingButton
					loading={attaching}
					onclick={attachReviewedBatch}
					disabled={!rightsConfirmed}
				>
					<CheckCircle2 class="size-4" /> เพิ่มเข้าคลังฟอนต์
				</LoadingButton>
			{/if}
		</div>
	{/if}
</section>
