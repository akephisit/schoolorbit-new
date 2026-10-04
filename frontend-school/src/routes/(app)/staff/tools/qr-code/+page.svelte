<script lang="ts">
	import { onDestroy } from 'svelte';
	import { QrCode, Download } from '@lucide/svelte';
	import PageShell from '#lib/components/app-layout/PageShell.svelte';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { getRequiredPublicSchoolInfo } from '#lib/api/school.js';
	import { downloadPublicFile } from '#lib/api/files.js';
	import { createQrPng, readLogo, validateQrText, QrTextError } from '#lib/tools/qr-code.js';

	type LogoMode = 'none' | 'school' | 'upload';
	let text = $state('');
	let logoMode = $state<LogoMode>('none');
	let logo = $state.raw<HTMLImageElement | null>(null);
	let logoPreview = $state('');
	let logoError = $state('');
	let textError = $state('');
	let generationError = $state('');
	let downloadError = $state('');
	let logoLoading = $state(false);
	let generating = $state(false);
	let downloading = $state(false);
	let result = $state.raw<{
		url: string;
		text: string;
		mode: LogoMode;
		logo: HTMLImageElement | null;
	} | null>(null);
	let logoAttempt = 0;
	let disposed = false;
	let controller: AbortController | null = null;
	const current = $derived(
		result !== null && result.text === text && result.mode === logoMode && result.logo === logo
	);

	function releaseLogo() {
		if (logoPreview) URL.revokeObjectURL(logoPreview);
		logoPreview = '';
		logo = null;
	}

	async function selectLogo(mode: LogoMode, file?: File) {
		const attempt = ++logoAttempt;
		controller?.abort();
		controller = new AbortController();
		const signal = controller.signal;
		logoMode = mode;
		releaseLogo();
		logoError = '';
		generationError = '';
		logoLoading = mode === 'school' || (mode === 'upload' && Boolean(file));
		if (!logoLoading) return;
		try {
			let blob: Blob;
			if (mode === 'school') {
				const school = await getRequiredPublicSchoolInfo({ signal });
				if (!school.logoFileId)
					throw new Error('โรงเรียนยังไม่ได้ตั้งค่าโลโก้ เลือกภาพจากเครื่องได้');
				blob = await downloadPublicFile(school.logoFileId, signal);
			} else if (file) {
				blob = file;
			} else {
				return;
			}
			const image = await readLogo(blob);
			if (disposed || attempt !== logoAttempt) return;
			logo = image;
			logoPreview = URL.createObjectURL(blob);
		} catch (error) {
			if (disposed || attempt !== logoAttempt) return;
			logoError =
				error instanceof TypeError ||
				(error instanceof Error && error.message === 'Failed to fetch')
					? 'โหลดโลโก้ไม่สำเร็จ กรุณาตรวจสอบการเชื่อมต่อแล้วลองใหม่ หรือเลือกภาพจากเครื่อง'
					: error instanceof Error
						? error.message
						: 'โหลดโลโก้ไม่สำเร็จ กรุณาลองใหม่';
		} finally {
			if (!disposed && attempt === logoAttempt) logoLoading = false;
		}
	}

	async function generate() {
		if (generating || logoLoading) return;
		textError = validateQrText(text) ?? '';
		generationError = '';
		downloadError = '';
		if (textError) return;
		if (logoMode !== 'none' && !logo) {
			logoError = 'กรุณาเลือกโลโก้ให้สำเร็จ หรือเลือกไม่ใส่โลโก้';
			return;
		}
		const snapshot = { text, mode: logoMode, logo };
		generating = true;
		try {
			const blob = await createQrPng(snapshot.text, snapshot.logo);
			if (disposed) return;
			if (result) URL.revokeObjectURL(result.url);
			result = { ...snapshot, url: URL.createObjectURL(blob) };
		} catch (error) {
			if (disposed) return;
			if (error instanceof QrTextError) textError = error.message;
			else generationError = 'สร้าง QR Code ไม่สำเร็จ กรุณาลองใหม่';
		} finally {
			if (!disposed) generating = false;
		}
	}

	async function download() {
		if (!current || !result || generating || logoLoading || downloading) return;
		downloading = true;
		downloadError = '';
		try {
			// Allow the pending state to paint before handing the exact preview file to the browser.
			await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
			if (disposed || !current || !result) return;
			const link = document.createElement('a');
			link.href = result.url;
			link.download = 'qr-code.png';
			link.click();
		} catch {
			downloadError = 'ดาวน์โหลดไม่สำเร็จ กรุณาลองใหม่';
		} finally {
			if (!disposed) downloading = false;
		}
	}

	onDestroy(() => {
		disposed = true;
		logoAttempt++;
		controller?.abort();
		releaseLogo();
		if (result) URL.revokeObjectURL(result.url);
	});
</script>

<PageShell
	title="สร้าง QR Code"
	description="สร้าง QR จากลิงก์หรือข้อความ พร้อมใส่โลโก้ตรงกลาง"
	backHref="/staff/tools"
	backLabel="เครื่องมือทั้งหมด"
	icon={QrCode}
>
	<div class="grid items-start gap-6 lg:grid-cols-2">
		<form
			class="min-w-0 space-y-6 rounded-xl border border-border bg-card p-4 sm:p-6"
			onsubmit={(event) => {
				event.preventDefault();
				void generate();
			}}
		>
			<div class="space-y-2">
				<Label for="qr-text">ลิงก์หรือข้อความ <span aria-hidden="true">*</span></Label>
				<Textarea
					id="qr-text"
					bind:value={text}
					rows={5}
					disabled={generating}
					aria-required="true"
					aria-invalid={Boolean(textError)}
					aria-describedby={textError ? 'qr-text-error' : 'qr-text-help'}
					oninput={() => {
						textError = '';
						generationError = '';
					}}
					placeholder="https://example.com หรือข้อความที่ต้องการ"
				/>
				<p id="qr-text-help" class="text-sm text-muted-foreground">รองรับข้อความภาษาไทยและลิงก์</p>
				{#if textError}<p id="qr-text-error" class="text-sm text-destructive" role="alert">
						{textError}
					</p>{/if}
			</div>

			<fieldset class="space-y-3" disabled={generating}>
				<legend class="mb-3 text-sm font-medium">โลโก้ตรงกลาง</legend>
				<div class="flex flex-wrap gap-2">
					<Button
						type="button"
						variant={logoMode === 'none' ? 'default' : 'outline'}
						aria-pressed={logoMode === 'none'}
						onclick={() => selectLogo('none')}>ไม่ใส่โลโก้</Button
					>
					<Button
						type="button"
						variant={logoMode === 'school' ? 'default' : 'outline'}
						aria-pressed={logoMode === 'school'}
						onclick={() => selectLogo('school')}>ใช้โลโก้โรงเรียน</Button
					>
					<Button
						type="button"
						variant={logoMode === 'upload' ? 'default' : 'outline'}
						aria-pressed={logoMode === 'upload'}
						onclick={() => selectLogo('upload')}>เลือกภาพจากเครื่อง</Button
					>
				</div>
				{#if logoMode === 'upload'}
					<div class="space-y-2">
						<Label for="qr-logo">ไฟล์โลโก้</Label>
						<Input
							id="qr-logo"
							type="file"
							accept="image/png,image/jpeg,image/webp"
							aria-invalid={Boolean(logoError)}
							aria-describedby={logoError ? 'qr-logo-help qr-logo-error' : 'qr-logo-help'}
							onchange={(event) => selectLogo('upload', event.currentTarget.files?.[0])}
						/>
						<p id="qr-logo-help" class="text-sm text-muted-foreground">
							PNG, JPEG หรือ WebP ไม่เกิน 5 MB
						</p>
					</div>
				{/if}
				<div aria-live="polite" aria-busy={logoLoading}>
					{#if logoLoading}<p class="text-sm text-muted-foreground" role="status">
							กำลังโหลดโลโก้...
						</p>{/if}
					{#if logoPreview}<img
							src={logoPreview}
							alt="โลโก้ที่เลือก"
							class="size-20 rounded-md border border-border bg-white object-contain p-2"
						/>{/if}
				</div>
				{#if logoError}
					<p id="qr-logo-error" class="text-sm text-destructive" role="alert">{logoError}</p>
					{#if logoMode === 'school'}<Button
							type="button"
							variant="outline"
							onclick={() => selectLogo('school')}>ลองโหลดโลโก้อีกครั้ง</Button
						>{/if}
				{/if}
			</fieldset>
			<p class="text-sm text-muted-foreground">
				ข้อความและภาพที่เลือกประมวลผลในเบราว์เซอร์ ไม่มีการบันทึกประวัติ
			</p>
			{#if generationError}<PageState
					variant="error"
					title="สร้าง QR Code ไม่สำเร็จ"
					description={generationError}
				/>{/if}
			<div class="flex justify-end">
				<LoadingButton
					type="submit"
					loading={generating}
					loadingLabel="กำลังสร้าง QR Code..."
					disabled={logoLoading}><QrCode class="size-4" />สร้าง QR Code</LoadingButton
				>
			</div>
		</form>

		<section aria-label="ตัวอย่าง QR Code" aria-busy={generating} class="min-w-0 space-y-4">
			<h2 class="text-lg font-semibold">ตัวอย่าง QR Code</h2>
			{#if result}
				<div class="rounded-xl border border-border bg-card p-4 sm:p-6">
					<img
						src={result.url}
						alt="QR Code ที่สร้าง"
						width="1024"
						height="1024"
						class="mx-auto aspect-square w-full max-w-sm rounded-md bg-white"
					/>
				</div>
				{#if !current}<p class="text-sm text-muted-foreground" role="status">
						ข้อมูลเปลี่ยนแล้ว กรุณาสร้าง QR Code ใหม่ก่อนดาวน์โหลด
					</p>{/if}
				<p class="text-sm text-muted-foreground">
					PNG ขนาด 1024 × 1024 พิกเซล · ทดลองสแกนก่อนนำไปใช้งาน
				</p>
				<LoadingButton
					type="button"
					onclick={download}
					loading={downloading}
					loadingLabel="กำลังดาวน์โหลด..."
					disabled={!current || generating || logoLoading}
					><Download class="size-4" />ดาวน์โหลด PNG</LoadingButton
				>
			{:else}
				<PageState
					title="ยังไม่มี QR Code"
					description="ใส่ลิงก์หรือข้อความ แล้วกดสร้าง QR Code เพื่อดูตัวอย่าง"
				/>
			{/if}
			{#if downloadError}<PageState
					variant="error"
					title="ดาวน์โหลดไม่สำเร็จ"
					description={downloadError}
				/>{/if}
		</section>
	</div>
</PageShell>
