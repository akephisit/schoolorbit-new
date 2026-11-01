<script lang="ts">
	import { onMount } from 'svelte';
	import { Send, Monitor, Download, QrCode } from '@lucide/svelte';
	import PageShell from '#lib/components/app-layout/PageShell.svelte';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Progress } from '#lib/components/ui/progress/index.js';
	import { createQrPng } from '#lib/tools/qr-code.js';
	import { FileTransferSession, initialTransferState } from '#lib/tools/file-transfer-session.js';
	import {
		formatFileSize,
		pairingCodeFromHash,
		pairingUrl,
		validateTransferFile
	} from '#lib/tools/file-transfer.js';

	let view = $state.raw(initialTransferState());
	let session: FileTransferSession | null = null;
	let label = $state('อุปกรณ์ของฉัน');
	let selectedDevice = $state('');
	let code = $state('');
	let file = $state.raw<File | null>(null);
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
	const busy = $derived(Boolean(view.pair) || view.pending);
	const incoming = $derived(view.pair?.recipient.id === view.snapshot?.device.id);
	const active = $derived(
		['connecting', 'sending', 'receiving', 'acknowledging'].includes(view.phase)
	);
	const percent = $derived(
		view.pair?.file.size
			? Math.floor((view.bytes / view.pair.file.size) * 100)
			: view.phase === 'complete'
				? 100
				: 0
	);
	const link = $derived(
		view.snapshot ? pairingUrl(origin, pathname, view.snapshot.pairingCode) : ''
	);
	const phaseLabels = {
		idle: 'พร้อมรับส่งไฟล์',
		requesting: 'กำลังส่งคำขอ',
		waiting: 'รอผู้รับตอบรับ',
		consent: 'มีคำขอรับไฟล์',
		connecting: 'กำลังเชื่อมต่อโดยตรง',
		sending: 'กำลังส่งไฟล์',
		receiving: 'กำลังรับไฟล์',
		acknowledging: 'รอผู้รับยืนยันว่าได้รับครบ',
		complete: 'รับส่งไฟล์สำเร็จ',
		cancelled: 'สิ้นสุดคำขอ',
		failed: 'รับส่งไฟล์ไม่สำเร็จ'
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
					file = null;
					fileError = '';
					selectedDevice = '';
					if (fileInput) fileInput.value = '';
				}
			}
			view = next;
		});
		const pagehide = () => session?.stop();
		window.addEventListener('pagehide', pagehide);
		return () => {
			disposed = true;
			qrAttempt++;
			session?.stop();
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
	function download() {
		if (!view.download) return;
		const anchor = document.createElement('a');
		anchor.href = view.download.url;
		anchor.download = view.download.name;
		anchor.click();
	}
	function selectFile(value: File | undefined) {
		file = value ?? null;
		fileError = file
			? (validateTransferFile({ name: file.name, size: file.size, mimeType: file.type }) ?? '')
			: '';
	}
	function send(byCode = false) {
		if (!file) {
			fileError = 'กรุณาเลือกไฟล์';
			return;
		}
		fileError =
			validateTransferFile({ name: file.name, size: file.size, mimeType: file.type }) ?? '';
		codeError = '';
		if (fileError) return;
		if (byCode) {
			const normalized = code.trim().toUpperCase();
			if (!/^[A-F0-9]{10}$/.test(normalized)) {
				codeError = 'กรุณาใส่รหัสจับคู่ที่แสดงบนอุปกรณ์ผู้รับ';
				return;
			}
			void session?.send(file, { type: 'code', code: normalized });
		} else if (selectedDevice)
			void session?.send(file, { type: 'device', deviceId: selectedDevice });
	}
</script>

<PageShell
	title="ส่งไฟล์ในเครือข่ายเดียวกัน"
	description="จับคู่อุปกรณ์ของบุคลากรในโรงเรียน แล้วส่งไฟล์โดยตรงระหว่างเบราว์เซอร์"
	backHref="/staff/tools"
	backLabel="เครื่องมือทั้งหมด"
	icon={Send}
>
	<div class="space-y-5">
		<p class="rounded-xl border bg-muted/40 p-4 text-sm text-muted-foreground">
			ใช้ Wi-Fi หรือ LAN เดียวกันที่อนุญาตให้อุปกรณ์ติดต่อกัน
			ไฟล์จะส่งโดยตรงและไม่ถูกเก็บบนเซิร์ฟเวอร์
			ระบบใช้เซิร์ฟเวอร์สำหรับค้นหาอุปกรณ์และจับคู่เท่านั้น เปิดหน้านี้ไว้ทั้งสองเครื่อง
			ผู้รับต้องตอบรับก่อนเริ่มส่ง
		</p>
		{#if view.error}<PageState
				variant="error"
				title={view.phase === 'failed' ? 'รับส่งไฟล์ไม่สำเร็จ' : 'สถานะการเชื่อมต่อ'}
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
									ชื่อจะแสดงให้บุคลากรที่เปิดเครื่องมือนี้ในโรงเรียนเห็น ไม่เกิน 60 ตัวอักษร
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
							ออนไลน์ในโรงเรียน · บุคลากรคนอื่นเลือกอุปกรณ์นี้ หรือใช้รหัสด้านล่าง
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
						<Button variant="outline" onclick={() => session?.stop()}>ปิดรับส่งไฟล์</Button>
					{/if}
				</Card.Content></Card.Root
			>
			<Card.Root
				role="region"
				aria-labelledby="send-heading"
				aria-busy={view.pending}
				class="min-w-0 gap-0 py-0"
				><Card.Content class="space-y-4 p-4 sm:p-6">
					<h2 id="send-heading" class="text-lg font-semibold">ส่งไฟล์</h2>
					<div class="space-y-2">
						<Label for="transfer-file">เลือกไฟล์</Label><Input
							id="transfer-file"
							bind:ref={fileInput}
							type="file"
							disabled={!online || busy}
							aria-invalid={Boolean(fileError)}
							aria-describedby="file-help file-error"
							onchange={(event) => selectFile(event.currentTarget.files?.[0])}
						/>
						<p id="file-help" class="text-sm text-muted-foreground">
							ครั้งละ 1 ไฟล์ ไม่เกิน 64 MiB ผู้รับต้องกดดาวน์โหลดเพื่อบันทึกไฟล์
						</p>
						<p
							id="file-error"
							role={fileError ? 'alert' : undefined}
							class="text-sm text-destructive"
						>
							{fileError}
						</p>
						{#if file && !fileError}<p class="break-words text-sm">
								{file.name} · {formatFileSize(file.size)}
							</p>{/if}
					</div>
					<fieldset class="space-y-3" disabled={!online || busy}>
						<legend class="mb-2 text-sm font-medium">อุปกรณ์ออนไลน์ในโรงเรียน</legend>
						{#if view.snapshot?.peers.length}
							<RadioGroup.Root
								bind:value={selectedDevice}
								disabled={!online || busy}
								aria-label="อุปกรณ์ผู้รับ"
								class="space-y-2"
								>{#each view.snapshot.peers as device (device.id)}<Label
										for={`recipient-${device.id}`}
										class="flex cursor-pointer items-start gap-3 rounded-lg border p-3"
										><RadioGroup.Item
											id={`recipient-${device.id}`}
											value={device.id}
											class="mt-1"
										/><span class="min-w-0"
											><span class="block break-words font-medium">{device.displayName}</span><span
												class="block break-words text-sm text-muted-foreground"
												>{device.label} · {device.id.slice(-8)}</span
											></span
										></Label
									>{/each}</RadioGroup.Root
							>
						{:else}<p class="text-sm text-muted-foreground">
								{online
									? 'ยังไม่มีอุปกรณ์อื่น เปิดเครื่องมือนี้บนอุปกรณ์ผู้รับ'
									: 'เปิดรับส่งไฟล์เพื่อค้นหาอุปกรณ์'}
							</p>{/if}
						<LoadingButton
							loading={view.phase === 'requesting'}
							loadingLabel="กำลังส่งคำขอ"
							disabled={!file ||
								Boolean(fileError) ||
								!view.snapshot?.peers.some((device) => device.id === selectedDevice)}
							onclick={() => send()}>ส่งคำขอไปยังอุปกรณ์ที่เลือก</LoadingButton
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
								disabled={!online || busy}
								autocomplete="off"
								autocapitalize="characters"
								placeholder="รหัสที่แสดงบนเครื่องผู้รับ"
								aria-invalid={Boolean(codeError)}
								aria-describedby="code-error"
							/>
							<p
								id="code-error"
								role={codeError ? 'alert' : undefined}
								class="text-sm text-destructive"
							>
								{codeError}
							</p>
						</div>
						<Button
							type="submit"
							variant="outline"
							disabled={!online || busy || !file || Boolean(fileError)}>ส่งคำขอด้วยรหัส</Button
						>
					</form>
				</Card.Content></Card.Root
			>
		</div>
		<Card.Root role="region" aria-labelledby="transfer-heading" class="gap-0 py-0"
			><Card.Content class="space-y-4 p-4 sm:p-6">
				<h2 id="transfer-heading" class="text-lg font-semibold">การรับส่งไฟล์</h2>
				<p role="status" aria-live="polite" class="font-medium">{phaseLabels[view.phase]}</p>
				{#if view.pair}
					<p class="break-words text-sm">
						{incoming ? 'จาก' : 'ถึง'}
						{incoming ? view.pair.initiator.displayName : view.pair.recipient.displayName} · {incoming
							? view.pair.initiator.label
							: view.pair.recipient.label}
					</p>
					<p class="break-words">{view.pair.file.name} · {formatFileSize(view.pair.file.size)}</p>
					{#if view.phase === 'consent'}<p class="text-sm text-muted-foreground">
							ตรวจสอบชื่อผู้ส่งและไฟล์ก่อนตอบรับ รับไฟล์จากผู้ที่คุณรู้จักเท่านั้น
						</p>
						<div class="flex flex-wrap gap-2">
							<Button variant="outline" disabled={view.pending} onclick={() => session?.cancel()}
								>ปฏิเสธ</Button
							><LoadingButton
								loading={view.pending}
								loadingLabel="กำลังตอบรับ"
								onclick={() => session?.accept()}>รับไฟล์</LoadingButton
							>
						</div>
					{:else if view.phase === 'complete'}
						<Button variant="outline" onclick={() => session?.finish()}>สิ้นสุดการเชื่อมต่อ</Button>
					{:else}<Button variant="outline" onclick={() => session?.cancel()}>ยกเลิกการรับส่ง</Button
						>{/if}
				{/if}
				{#if active}<Progress value={percent} aria-label="ความคืบหน้าการรับส่งไฟล์" />
					<p class="text-sm text-muted-foreground">
						{formatFileSize(view.bytes)} · {percent}%
					</p>{/if}
				{#if view.download}<div class="space-y-3">
						<p class="break-words text-sm">
							ได้รับ {view.download.name} · {formatFileSize(view.download.size)} ไฟล์อยู่ในหน่วยความจำของหน้านี้จนกว่าจะปิดหรือล้าง
						</p>
						<div class="flex flex-wrap gap-2">
							<Button onclick={download}><Download class="size-4" />ดาวน์โหลดไฟล์</Button><Button
								variant="outline"
								onclick={() => session?.discard()}>ล้างไฟล์ที่รับแล้ว</Button
							>
						</div>
					</div>{/if}
				{#if view.phase === 'idle'}<p class="text-sm text-muted-foreground">
						คำขอที่ส่งเข้ามาจะแสดงที่นี่ ไม่มีการรับไฟล์อัตโนมัติ
					</p>{/if}
			</Card.Content></Card.Root
		>
	</div>
</PageShell>
