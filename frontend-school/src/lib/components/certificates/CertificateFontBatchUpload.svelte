<script lang="ts">
	import {
		attachCertificateFontBatch,
		inspectCertificateFontUploads,
		type CertificateFontUploadInspectionFile,
		type CertificateFontUploadStatus,
		type CertificateTemplateDetail
	} from '$lib/api/certificates';
	import { deleteFile, uploadCertificateTemplateFile, type FileMetadata } from '$lib/api/files';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Table from '$lib/components/ui/table';
	import { AlertTriangle, CheckCircle2, FileType2, RefreshCw, Trash2, Upload } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	const MAX_FONT_BATCH_FILES = 40;

	type FontUploadState =
		| 'queued'
		| 'uploading'
		| 'uploaded'
		| 'upload_failed'
		| 'ready'
		| 'rejected';

	type FontUploadRow = {
		key: string;
		file: File;
		status: FontUploadState;
		metadata?: FileMetadata;
		inspection?: CertificateFontUploadInspectionFile;
		error?: string;
		cleaning?: boolean;
	};

	let {
		templateId,
		canUpdate,
		onpatched,
		onpendingchange
	}: {
		templateId: string;
		canUpdate: boolean;
		onpatched: (template: CertificateTemplateDetail) => void;
		onpendingchange: (pending: boolean) => void;
	} = $props();

	let rows = $state.raw<FontUploadRow[]>([]);
	let rightsConfirmed = $state(false);
	let batchError = $state<string | null>(null);
	let uploading = $state(false);
	let inspecting = $state(false);
	let attaching = $state(false);
	let cleaningAll = $state(false);
	let inputKey = $state(0);
	let reportedPending = false;

	const allRowsReady = $derived(
		rows.length > 0 && rows.every((row) => row.status === 'ready' && row.metadata)
	);
	const hasTemporaryUploads = $derived(
		rows.some((row) => row.status === 'uploading' || row.metadata !== undefined)
	);

	const inspectionLabels: Record<CertificateFontUploadStatus, string> = {
		ready: 'พร้อมแนบ',
		duplicate_selection: 'variant ซ้ำในชุดนี้',
		duplicate_existing: 'variant นี้แนบแล้ว',
		unsupported_variable: 'ไม่รองรับ variable font',
		unsupported_weight: 'น้ำหนักไม่รองรับ',
		missing_family: 'อ่านชื่อ family ไม่ได้',
		unavailable: 'ไฟล์ยังไม่พร้อมหรือไม่ใช่ฟอนต์'
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

	function replaceRow(key: string, patch: Partial<FontUploadRow>): void {
		rows = rows.map((row) => (row.key === key ? { ...row, ...patch } : row));
		reportPending();
	}

	function selectFonts(event: Event): void {
		const input = event.currentTarget as HTMLInputElement;
		const selected = Array.from(input.files ?? []);
		batchError = null;
		if (hasTemporaryUploads) {
			batchError = 'แนบหรือลบไฟล์ชั่วคราวชุดเดิมให้เสร็จก่อนเลือกชุดใหม่';
			inputKey += 1;
			return;
		}
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
			const result = await inspectCertificateFontUploads(templateId, {
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
		if (uploading || inspecting || attaching) return;
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
				const metadata = await uploadCertificateTemplateFile(
					row.file,
					'certificate_template_font',
					templateId
				);
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

	function removeRow(key: string): void {
		const row = rows.find((candidate) => candidate.key === key);
		if (!row || row.metadata || row.status === 'uploading') return;
		rows = rows.filter((candidate) => candidate.key !== key);
		if (rows.length === 0) {
			rightsConfirmed = false;
			inputKey += 1;
		}
		reportPending();
	}

	async function cleanupTemporaryRow(key: string): Promise<void> {
		const row = rows.find((candidate) => candidate.key === key);
		if (!row?.metadata || row.cleaning) return;
		replaceRow(key, { cleaning: true, error: undefined });
		try {
			await deleteFile(row.metadata.id, templateId);
			rows = rows.filter((candidate) => candidate.key !== key);
			batchError = null;
			if (rows.length === 0) {
				rightsConfirmed = false;
				inputKey += 1;
			}
		} catch (error) {
			replaceRow(key, {
				cleaning: false,
				error: asMessage(error, 'ลบไฟล์ชั่วคราวไม่สำเร็จ')
			});
			return;
		}
		reportPending();
	}

	async function cleanupAllTemporaryRows(): Promise<void> {
		if (cleaningAll) return;
		cleaningAll = true;
		for (const row of rows.filter((candidate) => candidate.metadata)) {
			await cleanupTemporaryRow(row.key);
		}
		cleaningAll = false;
	}

	async function attachReviewedBatch(): Promise<void> {
		if (!allRowsReady || !rightsConfirmed || attaching) return;
		const fileIds = rows.map((row) => row.metadata!.id);
		attaching = true;
		batchError = null;
		try {
			const updated = await attachCertificateFontBatch(templateId, {
				fileIds,
				rightsConfirmed: true
			});
			rows = [];
			rightsConfirmed = false;
			inputKey += 1;
			reportPending();
			onpatched(updated);
			toast.success(`เพิ่มฟอนต์ ${fileIds.length} ไฟล์แล้ว`);
		} catch (error) {
			batchError = asMessage(error, 'แนบชุดฟอนต์กับแม่แบบไม่สำเร็จ');
		} finally {
			attaching = false;
		}
	}

	function localStatus(row: FontUploadRow): string {
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
				return 'พร้อมแนบ';
			case 'rejected':
				return 'ยังแนบไม่ได้';
		}
	}
</script>

<div class="space-y-4 rounded-xl border bg-muted/15 p-4">
	<div class="flex items-center gap-2">
		<span class="grid size-9 place-items-center rounded-lg bg-violet-100 text-violet-700">
			<FileType2 class="size-5" />
		</span>
		<div>
			<h4 class="text-sm font-medium">เพิ่มชุดฟอนต์</h4>
			<p class="text-xs text-muted-foreground">
				เลือก TTF/OTF ได้ไม่เกิน {MAX_FONT_BATCH_FILES} ไฟล์ ระบบอ่าน family น้ำหนัก และตัวเอียงให้
			</p>
		</div>
	</div>

	<div class="space-y-2">
		<Label for={`certificate-font-batch-${templateId}`}>ไฟล์ฟอนต์</Label>
		{#key inputKey}
			<Input
				id={`certificate-font-batch-${templateId}`}
				type="file"
				accept=".ttf,.otf"
				multiple
				onchange={selectFonts}
				disabled={!canUpdate || uploading || inspecting || attaching || hasTemporaryUploads}
			/>
		{/key}
	</div>

	{#if batchError}
		<div
			class="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
		>
			<AlertTriangle class="mt-0.5 size-4 shrink-0" />
			<p>{batchError}</p>
		</div>
	{/if}

	{#if rows.length > 0}
		<div class="overflow-x-auto rounded-lg border bg-background">
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head>ไฟล์</Table.Head>
						<Table.Head>Family</Table.Head>
						<Table.Head>น้ำหนัก</Table.Head>
						<Table.Head>รูปแบบ</Table.Head>
						<Table.Head>สถานะ</Table.Head>
						<Table.Head class="text-right">จัดการ</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each rows as row (row.key)}
						<Table.Row>
							<Table.Cell class="max-w-52">
								<p class="truncate text-sm font-medium" title={row.file.name}>{row.file.name}</p>
								{#if row.error}
									<p class="mt-1 text-xs text-destructive">{row.error}</p>
								{/if}
							</Table.Cell>
							<Table.Cell>{row.inspection?.fontFamily ?? '—'}</Table.Cell>
							<Table.Cell>{row.inspection?.fontWeight ?? '—'}</Table.Cell>
							<Table.Cell>
								{row.inspection?.fontStyle === 'italic'
									? 'ตัวเอียง'
									: row.inspection?.fontStyle === 'normal'
										? 'ตัวปกติ'
										: '—'}
							</Table.Cell>
							<Table.Cell>
								<Badge variant={row.status === 'ready' ? 'secondary' : 'outline'}>
									{#if row.status === 'ready'}<CheckCircle2 class="size-3" />{/if}
									{localStatus(row)}
								</Badge>
							</Table.Cell>
							<Table.Cell class="text-right">
								<div class="flex justify-end gap-1">
									{#if row.status === 'upload_failed' || row.status === 'uploaded'}
										<Button
											size="sm"
											variant="ghost"
											onclick={() => retryRow(row.key)}
											disabled={uploading || inspecting || attaching}
										>
											<RefreshCw class="size-3.5" /> ลองใหม่
										</Button>
									{/if}
									{#if row.metadata}
										<LoadingButton
											size="sm"
											variant="ghost"
											loading={row.cleaning === true}
											onclick={() => cleanupTemporaryRow(row.key)}
										>
											<Trash2 class="size-3.5" /> ลบไฟล์ชั่วคราว
										</LoadingButton>
									{:else if row.status !== 'uploading'}
										<Button size="sm" variant="ghost" onclick={() => removeRow(row.key)}>
											นำออก
										</Button>
									{/if}
								</div>
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		</div>

		{#if allRowsReady}
			<label
				class="flex cursor-pointer items-start gap-3 rounded-lg border bg-background p-3 text-sm"
			>
				<Checkbox bind:checked={rightsConfirmed} class="mt-0.5" />
				<span>
					<strong class="font-medium">ยืนยันว่ามีสิทธิ์ใช้และฝังฟอนต์ทุกไฟล์ในชุดนี้</strong>
					<span class="mt-0.5 block text-xs leading-relaxed text-muted-foreground">
						ยืนยันเพียงครั้งเดียวก่อนแนบชุดฟอนต์แบบ atomic
					</span>
				</span>
			</label>
		{/if}

		<div class="flex flex-wrap gap-2">
			{#if rows.some((row) => !row.metadata)}
				<LoadingButton
					variant="outline"
					loading={uploading || inspecting}
					onclick={() => uploadSelectedRows()}
					disabled={!canUpdate || attaching}
				>
					<Upload class="size-4" /> อัปโหลดและตรวจสอบ
				</LoadingButton>
			{:else if rows.some((row) => row.status === 'uploaded')}
				<LoadingButton variant="outline" loading={inspecting} onclick={inspectUploadedRows}>
					<RefreshCw class="size-4" /> ตรวจอีกครั้ง
				</LoadingButton>
			{/if}
			<LoadingButton
				loading={attaching}
				onclick={attachReviewedBatch}
				disabled={!canUpdate || !allRowsReady || !rightsConfirmed || uploading || inspecting}
			>
				<CheckCircle2 class="size-4" /> แนบชุดฟอนต์
			</LoadingButton>
			{#if hasTemporaryUploads}
				<LoadingButton
					variant="outline"
					loading={cleaningAll}
					onclick={cleanupAllTemporaryRows}
					disabled={uploading || inspecting || attaching}
				>
					<Trash2 class="size-4" /> ลบไฟล์ชั่วคราวทั้งหมด
				</LoadingButton>
			{/if}
		</div>
	{/if}
</div>
