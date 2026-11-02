<script lang="ts">
	import { onMount } from 'svelte';
	import { Send, Monitor, Download, QrCode } from '@lucide/svelte';
	import PageShell from '#lib/components/app-layout/PageShell.svelte';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Progress } from '#lib/components/ui/progress/index.js';
	import { createQrPng } from '#lib/tools/qr-code.js';
	import {
		FileTransferSession,
		initialTransferState,
		isTransferTerminal,
		type TransferPhase,
		type TransferJob
	} from '#lib/tools/file-transfer-session.js';
	import {
		batchBytes,
		fileMetadata,
		formatFileSize,
		pairingCodeFromHash,
		pairingUrl,
		validateTransferBatch,
		TRANSFER_PAGE_SIZE
	} from '#lib/tools/file-transfer.js';

	let view = $state.raw(initialTransferState());
	let session: FileTransferSession | null = null;
	let label = $state('อุปกรณ์ของฉัน');
	let selectedDevices = $state<string[]>([]);
	let code = $state('');
	let files = $state.raw<File[]>([]);
	let fileInput = $state.raw<HTMLInputElement | null>(null);
	let fileError = $state('');
	let codeError = $state('');
	let qrUrl = $state('');
	let qrError = $state('');
	let qrPending = $state(false);
	let copied = $state(false);
	let origin = $state('');
	let pathname = $state('');
	let qrAttempt = 0;
	let disposed = false;
	const online = $derived(view.status === 'online');
	const metadata = $derived(files.map(fileMetadata));
	const sourceBusy = $derived(
		view.jobs.some(
			(job) => job.direction === 'outgoing' && (!isTransferTerminal(job.phase) || job.slotReserved)
		)
	);
	const receiving = $derived(
		view.jobs.some((job) => job.direction === 'incoming' && job.slotReserved)
	);
	const downloadActive = $derived(
		view.jobs.some(
			(job) =>
				job.direction === 'incoming' && !isTransferTerminal(job.phase) && job.completedFiles > 0
		)
	);
	const link = $derived(
		view.snapshot ? pairingUrl(origin, pathname, view.snapshot.pairingCode) : ''
	);
	const phaseLabels: Record<TransferPhase, string> = {
		queued: 'รอคิวส่งคำขอ',
		requesting: 'กำลังส่งคำขอ',
		waiting: 'รอผู้รับตอบรับ',
		consent: 'มีคำขอรับชุดไฟล์',
		connecting: 'กำลังเชื่อมต่อโดยตรง',
		manifest: 'กำลังรับส่งรายการไฟล์',
		review: 'รอตรวจสอบรายการและยืนยันรับไฟล์',
		sending: 'กำลังส่งไฟล์',
		receiving: 'กำลังรับไฟล์',
		acknowledging: 'รอผู้รับยืนยันว่าได้รับครบ',
		complete: 'รับส่งชุดไฟล์สำเร็จ',
		declined: 'ผู้รับปฏิเสธคำขอ',
		cancelled: 'สิ้นสุดคำขอ',
		failed: 'รับส่งชุดไฟล์ไม่สำเร็จ'
	};
	onMount(() => {
		origin = window.location.origin;
		pathname = window.location.pathname;
		code = pairingCodeFromHash(window.location.hash);
		if (window.location.hash.startsWith('#code='))
			window.history.replaceState(window.history.state, '', `${pathname}${window.location.search}`);
		session = new FileTransferSession((next) => {
			if (disposed) return;
			if (next.snapshot?.device.id !== view.snapshot?.device.id) {
				clearQr();
				if (view.snapshot) {
					files = [];
					fileError = '';
					selectedDevices = [];
					if (fileInput) fileInput.value = '';
				}
			}
			view = next;
		});
		const pagehide = () => session?.dispose();
		window.addEventListener('pagehide', pagehide);
		return () => {
			disposed = true;
			qrAttempt++;
			session?.dispose();
			clearQr();
			window.removeEventListener('pagehide', pagehide);
		};
	});
	function clearQr() {
		qrAttempt++;
		if (qrUrl) URL.revokeObjectURL(qrUrl);
		qrUrl = '';
		qrError = '';
		qrPending = false;
		copied = false;
	}
	async function showQr() {
		if (!link || qrPending) return;
		const currentLink = link,
			attempt = ++qrAttempt;
		qrPending = true;
		qrError = '';
		try {
			const blob = await createQrPng(currentLink, null);
			if (disposed || attempt !== qrAttempt || currentLink !== link) return;
			if (qrUrl) URL.revokeObjectURL(qrUrl);
			qrUrl = URL.createObjectURL(blob);
		} catch {
			if (!disposed && attempt === qrAttempt) qrError = 'สร้าง QR ไม่สำเร็จ ใช้รหัสจับคู่แทนได้';
		} finally {
			if (!disposed && attempt === qrAttempt) qrPending = false;
		}
	}
	async function copyLink() {
		copied = false;
		qrError = '';
		try {
			await navigator.clipboard.writeText(link);
			if (!disposed) copied = true;
		} catch {
			qrError = 'คัดลอกลิงก์ไม่สำเร็จ ใช้รหัสจับคู่หรือ QR แทนได้';
		}
	}
	function selectFiles(list: FileList | null) {
		files = list ? Array.from(list) : [];
		fileError = view.snapshot
			? (validateTransferBatch(files.map(fileMetadata), view.snapshot.limits) ?? '')
			: '';
	}
	function clearFiles() {
		if (sourceBusy) return;
		session?.clearSource();
		files = [];
		fileError = '';
		if (fileInput) fileInput.value = '';
	}
	function send(byCode = false) {
		if (!view.snapshot) return;
		fileError = validateTransferBatch(metadata, view.snapshot.limits) ?? '';
		codeError = '';
		if (fileError) return;
		if (byCode) {
			const normalized = code.trim().toUpperCase();
			if (!/^[A-F0-9]{10}$/.test(normalized)) {
				codeError = 'กรุณาใส่รหัสจับคู่ที่แสดงบนอุปกรณ์ผู้รับ';
				return;
			}
			session?.send(files, [{ type: 'code', code: normalized }]);
		} else
			session?.send(
				files,
				selectedDevices
					.filter((id) => view.snapshot?.peers.some((peer) => peer.id === id))
					.map((deviceId) => ({ type: 'device', deviceId }))
			);
	}
	function percent(job: TransferJob) {
		const total = job.batch.totalBytes;
		return total
			? Math.min(100, Math.floor((job.bytes / total) * 100))
			: Math.floor((job.completedFiles / job.batch.fileCount) * 100);
	}
</script>

<PageShell
	title="ส่งไฟล์ในเครือข่ายเดียวกัน"
	description="เลือกหลายไฟล์และหลายผู้รับ ตรวจสอบรายการก่อนยืนยันรับไฟล์ แล้วดาวน์โหลดรวมเป็น ZIP ได้"
	backHref="/staff/tools"
	backLabel="เครื่องมือทั้งหมด"
	icon={Send}
>
	<div class="space-y-5">
		<p class="rounded-xl border bg-muted/40 p-4 text-sm text-muted-foreground">
			ใช้ Wi-Fi หรือ LAN เดียวกันที่อนุญาตให้อุปกรณ์ติดต่อกัน
			ไฟล์ส่งโดยตรงและไม่ถูกเก็บบนเซิร์ฟเวอร์ เปิดหน้านี้ไว้ทั้งสองเครื่อง
			แต่ละผู้รับรับไฟล์ตามลำดับของชุด การรอหรือยกเลิกผู้รับหนึ่งไม่หยุดผู้รับอื่น
		</p>
		{#if view.error}<PageState
				variant="error"
				title="สถานะการเชื่อมต่อ"
				description={view.error}
			/>{/if}
		<div class="grid items-start gap-5 lg:grid-cols-2">
			<Card.Root role="region" aria-labelledby="device-heading" class="min-w-0 gap-0 py-0"
				><Card.Content class="space-y-4 p-4 sm:p-6">
					<h2 id="device-heading" class="flex items-center gap-2 text-lg font-semibold">
						<Monitor class="size-5" />อุปกรณ์นี้
					</h2>
					{#if !online}
						<form
							class="space-y-4"
							onsubmit={(event) => {
								event.preventDefault();
								void session?.start(label);
							}}
						>
							<div class="space-y-2">
								<Label for="device-label">ชื่ออุปกรณ์</Label><Input
									id="device-label"
									bind:value={label}
									disabled={view.status === 'connecting'}
									placeholder="เช่น โน้ตบุ๊กห้องพักครู"
								/>
								<p class="text-sm text-muted-foreground">
									ชื่อแสดงให้บุคลากรที่เปิดเครื่องมือนี้ในโรงเรียนเห็น ไม่เกิน 60 ตัวอักษร
								</p>
							</div>
							<LoadingButton
								type="submit"
								loading={view.status === 'connecting'}
								loadingLabel="กำลังเปิดรับส่งไฟล์">เปิดรับส่งไฟล์</LoadingButton
							>
						</form>
					{:else if view.snapshot}
						<p class="break-words font-medium">
							{view.snapshot.device.label}
							<span class="text-sm font-normal text-muted-foreground"
								>({view.snapshot.device.id.slice(-8)})</span
							>
						</p>
						<p class="text-sm text-muted-foreground">
							ออนไลน์ในโรงเรียน · ให้อุปกรณ์ส่งเลือกชื่อเครื่องนี้ หรือใช้รหัสด้านล่าง
						</p>
						<div class="space-y-2">
							<p class="text-sm font-medium">รหัสจับคู่อุปกรณ์นี้</p>
							<p
								class="break-all rounded-lg bg-muted p-3 font-mono text-2xl tracking-widest"
								data-testid="pairing-code"
							>
								{view.snapshot.pairingCode}
							</p>
						</div>
						<div class="flex flex-wrap gap-2">
							<LoadingButton
								variant="outline"
								onclick={showQr}
								loading={qrPending}
								loadingLabel="กำลังสร้าง QR"><QrCode class="size-4" />แสดง QR จับคู่</LoadingButton
							><Button variant="outline" onclick={copyLink}>คัดลอกลิงก์จับคู่</Button>
						</div>
						{#if copied}<p role="status" class="text-sm text-muted-foreground">
								คัดลอกลิงก์แล้ว
							</p>{/if}
						{#if qrUrl}<img
								src={qrUrl}
								alt="QR สำหรับจับคู่อุปกรณ์นี้"
								class="size-56 max-w-full rounded-lg border bg-white"
							/>
							<p class="text-sm text-muted-foreground">
								สแกนด้วยกล้องของอุปกรณ์ที่จะส่งไฟล์ แล้วเข้าสู่ระบบโรงเรียนเดียวกัน QR
								และรหัสช่วยจับคู่เท่านั้น ไม่สามารถแก้การปิดกั้นเครือข่ายได้
							</p>{/if}
						{#if qrError}<p role="alert" class="text-sm text-destructive">{qrError}</p>{/if}
						<Button variant="outline" onclick={() => session?.stop()}>ปิดรับส่งไฟล์ทั้งหมด</Button>
					{/if}
				</Card.Content></Card.Root
			>
			<Card.Root role="region" aria-labelledby="send-heading" class="min-w-0 gap-0 py-0"
				><Card.Content class="space-y-4 p-4 sm:p-6">
					<h2 id="send-heading" class="text-lg font-semibold">ส่งชุดไฟล์</h2>
					<div class="space-y-2">
						<Label for="transfer-files">เลือกไฟล์หลายไฟล์</Label><Input
							id="transfer-files"
							bind:ref={fileInput}
							type="file"
							multiple
							disabled={!online || sourceBusy || receiving}
							aria-invalid={Boolean(fileError)}
							aria-describedby="file-help file-error"
							onchange={(event) => selectFiles(event.currentTarget.files)}
						/>
						<p id="file-help" class="text-sm text-muted-foreground">
							{#if view.snapshot}ไม่กำหนดเพดานจำนวนหรือขนาดไฟล์
								ขึ้นกับพื้นที่จัดเก็บและความสามารถของเบราว์เซอร์ ส่งพร้อมกันได้ {view.snapshot
									.limits.maxOutgoingTransfers} ผู้รับ ผู้รับที่เหลือรอคิว{:else}เปิดรับส่งไฟล์เพื่อค้นหาอุปกรณ์{/if}
						</p>
						{#if fileError}<p id="file-error" role="alert" class="text-sm text-destructive">
								{fileError}
							</p>{/if}
						{#if files.length}<p class="text-sm font-medium" data-testid="source-summary">
								เลือก {files.length} ไฟล์ · {formatFileSize(batchBytes(metadata))}
							</p>
							<ol class="space-y-1 text-sm">
								{#each metadata.slice(0, TRANSFER_PAGE_SIZE) as file, index (index)}<li
										class="break-words"
									>
										{index + 1}. {file.name} · {formatFileSize(file.size)}
									</li>{/each}
							</ol>
							{#if metadata.length > TRANSFER_PAGE_SIZE}<p class="text-sm text-muted-foreground">
									แสดง {TRANSFER_PAGE_SIZE} รายการแรกจาก {metadata.length} ไฟล์ ผู้รับตรวจสอบรายการทั้งหมดได้ก่อนรับ
								</p>{/if}
							<Button variant="outline" disabled={sourceBusy} onclick={clearFiles}
								>ล้างชุดไฟล์ที่เลือก</Button
							>{/if}
					</div>
					<fieldset class="space-y-3" disabled={!online || receiving}>
						<legend class="mb-2 text-sm font-medium">อุปกรณ์ออนไลน์ในโรงเรียน</legend>
						{#if view.snapshot?.peers.length}<div class="space-y-2">
								{#each view.snapshot.peers as device (device.id)}<Label
										for={`recipient-${device.id}`}
										class="flex cursor-pointer items-start gap-3 rounded-lg border p-3"
										><Checkbox
											id={`recipient-${device.id}`}
											checked={selectedDevices.includes(device.id)}
											onCheckedChange={(checked) => {
												selectedDevices = checked
													? [...new Set([...selectedDevices, device.id])]
													: selectedDevices.filter((id) => id !== device.id);
											}}
											class="mt-1"
										/><span class="min-w-0"
											><span class="block break-words font-medium">{device.displayName}</span><span
												class="block break-words text-sm text-muted-foreground"
												>{device.label} · {device.id.slice(-8)}</span
											></span
										></Label
									>{/each}
							</div>{:else}<p class="text-sm text-muted-foreground">
								{online
									? 'ยังไม่มีอุปกรณ์อื่น เปิดเครื่องมือนี้บนอุปกรณ์ผู้รับ'
									: 'เปิดรับส่งไฟล์เพื่อค้นหาอุปกรณ์'}
							</p>{/if}
						<Button
							disabled={!files.length ||
								Boolean(fileError) ||
								!selectedDevices.some((id) => view.snapshot?.peers.some((peer) => peer.id === id))}
							onclick={() => send()}>ส่งชุดไฟล์ให้ผู้รับที่เลือก</Button
						>
					</fieldset>
					<form
						class="space-y-3 border-t pt-4"
						onsubmit={(event) => {
							event.preventDefault();
							send(true);
						}}
					>
						<div class="space-y-2">
							<Label for="recipient-code">หรือใช้รหัสอุปกรณ์ผู้รับ</Label><Input
								id="recipient-code"
								bind:value={code}
								disabled={!online || receiving}
								autocomplete="off"
								autocapitalize="characters"
								placeholder="รหัสที่แสดงบนเครื่องผู้รับ"
								aria-invalid={Boolean(codeError)}
								aria-describedby="code-error"
							/>{#if codeError}<p id="code-error" role="alert" class="text-sm text-destructive">
									{codeError}
								</p>{/if}
						</div>
						<Button
							type="submit"
							variant="outline"
							disabled={!online || receiving || !files.length || Boolean(fileError)}
							>เพิ่มผู้รับด้วยรหัส</Button
						>
						<p class="text-sm text-muted-foreground">
							เพิ่มผู้รับของไฟล์ชุดเดียวกันได้ระหว่างรอคิวหรือกำลังส่ง
						</p>
					</form>
				</Card.Content></Card.Root
			>
		</div>
		<section aria-labelledby="transfers-heading" class="space-y-4">
			<h2 id="transfers-heading" class="text-lg font-semibold">การรับส่งไฟล์</h2>
			{#if !view.jobs.length}<p class="text-sm text-muted-foreground">
					คำขอแต่ละผู้รับจะแสดงที่นี่ ไม่มีการรับไฟล์อัตโนมัติ
				</p>{/if}
			<div class="grid items-start gap-4 xl:grid-cols-2">
				{#each view.jobs as job (job.id)}
					<Card.Root
						role="region"
						aria-labelledby={`job-${job.id}`}
						class="min-w-0 gap-0 py-0"
						data-testid="transfer-job"
						data-direction={job.direction}
						data-phase={job.phase}
						><Card.Content class="space-y-3 p-4 sm:p-6">
							<h3 id={`job-${job.id}`} class="break-words font-semibold">
								{job.direction === 'incoming' ? 'จาก' : 'ถึง'}
								{job.label}
							</h3>
							<p role="status" aria-live="polite" class="font-medium">{phaseLabels[job.phase]}</p>
							<p class="text-sm">
								{job.batch.fileCount} ไฟล์ · {formatFileSize(job.batch.totalBytes)}
							</p>
							{#if job.error}<p role="alert" class="text-sm text-destructive">{job.error}</p>{/if}
							{#if job.storageWarning}<p role="status" class="text-sm text-muted-foreground">
									{job.storageWarning}
								</p>{/if}
							{#if job.cleanupError}<p role="alert" class="text-sm text-destructive">
									{job.cleanupError}
								</p>{/if}
							{#if job.phase === 'consent'}
								<p class="text-sm text-muted-foreground">
									ตรวจสอบผู้ส่งก่อนเปิดดูรายการ การดูรายการยังไม่อนุญาตให้รับเนื้อหาไฟล์
								</p>
								{#if !view.receivingSupported}<p role="alert" class="text-sm text-destructive">
										เบราว์เซอร์นี้ไม่รองรับพื้นที่เก็บไฟล์สำหรับการรับ ใช้ Chrome หรือ Edge
										รุ่นปัจจุบันผ่าน HTTPS อุปกรณ์นี้ยังส่งไฟล์ได้
									</p>{/if}
								{#if view.downloadCount}<p class="text-sm text-muted-foreground">
										ยังมีไฟล์ชุดก่อนอยู่ ดาวน์โหลดให้เรียบร้อยแล้วล้างก่อนรับชุดใหม่
									</p>
									<Button
										variant="outline"
										disabled={downloadActive}
										onclick={() => session?.clearDownloads()}>ล้างไฟล์เดิมก่อนรับชุดใหม่</Button
									>{/if}
								<div class="flex flex-wrap gap-2">
									<Button
										variant="outline"
										disabled={job.pending}
										onclick={() => session?.cancel(job.id)}>ปฏิเสธ</Button
									><LoadingButton
										loading={job.pending}
										loadingLabel="กำลังเปิดรายการ"
										disabled={Boolean(view.downloadCount) || !view.receivingSupported}
										onclick={() => session?.acceptManifest(job.id)}>ดูรายการไฟล์</LoadingButton
									>
								</div>
							{:else if job.phase === 'review'}
								{#if job.direction === 'incoming'}
									<ol class="space-y-1 text-sm" aria-busy={job.manifestPending}>
										{#each job.files as file, index (index)}<li class="break-words">
												{job.manifestPage * TRANSFER_PAGE_SIZE + index + 1}. {file.name} · {formatFileSize(
													file.size
												)}
											</li>{/each}
									</ol>
									{#if job.batch.fileCount > TRANSFER_PAGE_SIZE}<div
											class="flex flex-wrap items-center gap-2"
										>
											<Button
												variant="outline"
												disabled={job.manifestPending || job.manifestPage === 0}
												onclick={() => session?.showManifestPage(job.id, job.manifestPage - 1)}
												>รายการก่อนหน้า</Button
											><span class="text-sm"
												>หน้า {job.manifestPage + 1}/{Math.ceil(
													job.batch.fileCount / TRANSFER_PAGE_SIZE
												)}</span
											><Button
												variant="outline"
												disabled={job.manifestPending ||
													(job.manifestPage + 1) * TRANSFER_PAGE_SIZE >= job.batch.fileCount}
												onclick={() => session?.showManifestPage(job.id, job.manifestPage + 1)}
												>รายการถัดไป</Button
											>
										</div>{/if}
									<p class="text-sm text-muted-foreground">
										ตรวจสอบรายการทั้งหมดก่อนตอบรับ การยืนยันครั้งเดียวอนุญาตให้รับชุดนี้ตามลำดับ
										ใช้พื้นที่ชั่วคราวของเบราว์เซอร์และต้องเปิดหน้านี้ไว้
									</p>
									<LoadingButton
										loading={job.pending}
										loadingLabel="กำลังตรวจสอบพื้นที่"
										disabled={job.manifestPending || !job.files.length}
										onclick={() => session?.acceptFiles(job.id)}>รับไฟล์ทั้งชุด</LoadingButton
									>
								{:else}<p class="text-sm text-muted-foreground">
										รอผู้รับตรวจสอบรายการและยืนยันรับไฟล์ ยังไม่ได้ส่งเนื้อหาไฟล์
									</p>{/if}
								<Button variant="outline" onclick={() => session?.cancel(job.id)}
									>ยกเลิกชุดนี้</Button
								>
							{:else if !isTransferTerminal(job.phase)}
								{#if ['sending', 'receiving', 'acknowledging', 'connecting', 'manifest'].includes(job.phase)}<Progress
										value={percent(job)}
										aria-label={`ความคืบหน้า ${job.label}`}
									/>
									<p class="text-sm text-muted-foreground">
										{formatFileSize(job.bytes)} · {job.megabytesPerSecond.toFixed(2)} MB/s · {percent(
											job
										)}% · รับครบ {job.completedFiles}/{job.batch.fileCount} ไฟล์
									</p>{/if}
								<Button variant="outline" onclick={() => session?.cancel(job.id)}
									>{job.phase === 'queued'
										? 'ยกเลิกคิว'
										: job.direction === 'incoming'
											? 'ยกเลิกการรับชุดนี้'
											: 'ยกเลิกผู้รับนี้'}</Button
								>
							{:else}
								{#if job.slotReserved}<Button
										variant="outline"
										onclick={() => session?.finish(job.id)}>สิ้นสุดคำขอ</Button
									>{/if}
								{#if job.direction === 'outgoing' && job.phase !== 'complete'}<Button
										variant="outline"
										disabled={!online || !files.length || receiving}
										onclick={() => session?.retry(job.id, files)}>ส่งคำขอใหม่</Button
									>{/if}
							{/if}
						</Card.Content></Card.Root
					>
				{/each}
			</div>
		</section>
		{#if view.downloadCount}<Card.Root
				role="region"
				aria-labelledby="downloads-heading"
				class="gap-0 py-0"
				><Card.Content class="space-y-4 p-4 sm:p-6">
					<h2 id="downloads-heading" class="text-lg font-semibold">ไฟล์ที่รับครบแล้ว</h2>
					<p class="text-sm text-muted-foreground">
						ไฟล์อยู่ในพื้นที่ชั่วคราวของเบราว์เซอร์ กดดาวน์โหลดเพื่อบันทึก
						ปิดหน้าหรือออกจากระบบจะล้างไฟล์ ไฟล์ที่รับครบแล้วยังคงอยู่แม้ส่วนที่เหลือถูกยกเลิก
					</p>
					<p class="text-sm">
						รับครบ {view.downloadCount} ไฟล์ · {formatFileSize(view.downloadBytes)}
					</p>
					<div class="flex flex-wrap gap-2">
						<LoadingButton
							loading={view.archive.pending}
							loadingLabel="กำลังสร้าง ZIP"
							disabled={downloadActive}
							onclick={() => session?.downloadAll()}
							><Download class="size-4" />ดาวน์โหลดทั้งหมดเป็น ZIP</LoadingButton
						>{#if view.archive.pending}<Button
								variant="outline"
								onclick={() => session?.cancelArchive()}>ยกเลิกการสร้าง ZIP</Button
							>{/if}
					</div>
					<p class="text-sm text-muted-foreground">
						สร้าง ZIP ได้เมื่อสิ้นสุดการรับชุดนี้ ZIP จะเติมลำดับหน้าชื่อเพื่อแยกไฟล์ชื่อซ้ำ
						และต้องมีพื้นที่ว่างเพิ่มใกล้เคียงขนาดไฟล์ที่รับ ไฟล์เดิมยังอยู่หากสร้าง ZIP ไม่สำเร็จ
					</p>
					{#if view.archive.pending}<p role="status" class="text-sm">
							เตรียม ZIP แล้ว {formatFileSize(view.archive.bytes)}
						</p>{/if}
					{#if view.archive.error}<p role="alert" class="text-sm text-destructive">
							{view.archive.error}
						</p>{/if}
					{#if view.downloadError}<p role="alert" class="text-sm text-destructive">
							{view.downloadError}
						</p>{/if}
					<ol class="space-y-3" aria-busy={view.downloadPending}>
						{#each view.downloads as file (file.id)}<li
								class="flex flex-wrap items-center justify-between gap-3 rounded-lg border p-3"
							>
								<p class="min-w-0 break-words text-sm">
									{file.index + 1}. {file.name} · {formatFileSize(file.size)}
								</p>
								<Button
									onclick={() => session?.downloadFile(file.index)}
									aria-label={`ดาวน์โหลด ${file.index + 1} ${file.name}`}
									><Download class="size-4" />ดาวน์โหลด</Button
								>
							</li>{/each}
					</ol>
					{#if view.downloadCount > TRANSFER_PAGE_SIZE}<div
							class="flex flex-wrap items-center gap-2"
						>
							<Button
								variant="outline"
								disabled={view.downloadPending || view.downloadPage === 0}
								onclick={() => session?.showDownloadsPage(view.downloadPage - 1)}
								>ไฟล์ก่อนหน้า</Button
							><span class="text-sm"
								>หน้า {view.downloadPage + 1}/{Math.ceil(
									view.downloadCount / TRANSFER_PAGE_SIZE
								)}</span
							><Button
								variant="outline"
								disabled={view.downloadPending ||
									(view.downloadPage + 1) * TRANSFER_PAGE_SIZE >= view.downloadCount}
								onclick={() => session?.showDownloadsPage(view.downloadPage + 1)}>ไฟล์ถัดไป</Button
							>
						</div>{/if}
					<Button
						variant="outline"
						disabled={downloadActive}
						onclick={() => session?.clearDownloads()}>ล้างไฟล์ที่รับแล้วทั้งหมด</Button
					>
				</Card.Content></Card.Root
			>{/if}
	</div>
</PageShell>
