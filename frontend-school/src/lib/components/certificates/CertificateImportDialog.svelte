<script lang="ts">
	import { LoadingButton } from '$lib/components/app-state';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import {
		downloadCertificateCsvTemplate,
		downloadCertificateXlsxTemplate
	} from '$lib/certificates/import-template';
	import {
		CERTIFICATE_IMPORT_HEADERS,
		parseCertificateImport,
		type ParsedCertificateImport
	} from '$lib/certificates/importer';
	import { Download, FileSpreadsheet, Upload } from 'lucide-svelte';

	let {
		open,
		busy = false,
		onopenchange,
		onimport
	}: {
		open: boolean;
		busy?: boolean;
		onopenchange: (open: boolean) => void;
		onimport: (parsed: ParsedCertificateImport) => Promise<void>;
	} = $props();

	let parsed = $state.raw<ParsedCertificateImport | null>(null);
	let parsing = $state(false);
	let downloadingXlsx = $state(false);
	let error = $state('');

	const customHeaders = $derived(
		parsed?.headers.filter(
			(header) => !CERTIFICATE_IMPORT_HEADERS.some((standard) => standard === header)
		) ?? []
	);

	async function handleFile(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		parsed = null;
		error = '';
		if (!file) return;
		parsing = true;
		try {
			parsed = await parseCertificateImport(file);
		} catch (parseError) {
			error = parseError instanceof Error ? parseError.message : 'อ่านไฟล์รายชื่อไม่สำเร็จ';
		} finally {
			parsing = false;
		}
	}

	async function downloadXlsx() {
		downloadingXlsx = true;
		try {
			await downloadCertificateXlsxTemplate();
		} finally {
			downloadingXlsx = false;
		}
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>นำเข้า Excel/CSV</Dialog.Title>
			<Dialog.Description>
				ไฟล์จะถูกอ่านในเบราว์เซอร์ และส่งเฉพาะค่าจากแต่ละแถวไปตรวจสอบกับระบบ
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-5">
			<section class="rounded-lg border bg-muted/30 p-4">
				<div class="flex flex-wrap items-center justify-between gap-3">
					<div>
						<p class="font-medium">ดาวน์โหลดไฟล์ตัวอย่าง</p>
						<p class="text-xs text-muted-foreground">
							หัวคอลัมน์มาตรฐานเรียงตามรูปแบบที่ระบบรองรับ
						</p>
					</div>
					<div class="flex flex-wrap gap-2">
						<Button variant="outline" size="sm" onclick={downloadCertificateCsvTemplate}>
							<Download class="size-4" /> CSV (UTF-8)
						</Button>
						<LoadingButton
							variant="outline"
							size="sm"
							loading={downloadingXlsx}
							onclick={downloadXlsx}
						>
							<FileSpreadsheet class="size-4" /> Excel
						</LoadingButton>
					</div>
				</div>
			</section>

			<div class="space-y-2">
				<label for="certificate-import-file" class="text-sm font-medium">เลือกไฟล์รายชื่อ</label>
				<Input
					id="certificate-import-file"
					type="file"
					accept=".xlsx,.csv,text/csv,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
					disabled={busy || parsing}
					onchange={handleFile}
				/>
				<p class="text-xs text-muted-foreground">
					รองรับ .xlsx หนึ่งชีต หรือ .csv ที่เข้ารหัส UTF-8
					โดยเลขที่มีศูนย์นำหน้าควรกำหนดเป็นข้อความ
				</p>
			</div>

			{#if parsing}
				<div
					class="flex items-center gap-2 rounded-lg border border-blue-200 bg-blue-50 px-4 py-3 text-sm text-blue-800"
				>
					<Upload class="size-4 animate-pulse" /> กำลังอ่านไฟล์ในเบราว์เซอร์...
				</div>
			{:else if error}
				<div
					role="alert"
					class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800"
				>
					{error}
				</div>
			{:else if parsed}
				<div
					class="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-900"
				>
					<p class="font-semibold">
						พร้อมนำเข้า {parsed.rows.length.toLocaleString('th-TH')} รายการ
					</p>
					<p class="mt-1 text-xs">
						ชนิดไฟล์ {parsed.source.toUpperCase()} · {parsed.headers.length} คอลัมน์
						{#if customHeaders.length > 0}
							· ตัวแปรเพิ่มเติม {customHeaders.join(', ')}
						{/if}
					</p>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => onopenchange(false)}>ยกเลิก</Button>
			<LoadingButton
				loading={busy}
				disabled={!parsed || parsing}
				onclick={() => parsed && onimport(parsed)}
			>
				<Upload class="size-4" />
				{parsed ? `นำเข้า ${parsed.rows.length.toLocaleString('th-TH')} รายการ` : 'นำเข้ารายชื่อ'}
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
