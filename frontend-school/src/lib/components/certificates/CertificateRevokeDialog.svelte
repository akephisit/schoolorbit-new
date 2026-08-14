<script lang="ts">
	import {
		revokeIssuedCertificate,
		type IssuedCertificateSummary,
		type RevokeCertificateResult
	} from '$lib/api/certificates';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Textarea } from '$lib/components/ui/textarea';
	import { AlertTriangle, FilePlus2, ShieldAlert } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let {
		open,
		certificate,
		canRevoke = false,
		onopenchange,
		onrevoked
	}: {
		open: boolean;
		certificate: IssuedCertificateSummary;
		canRevoke?: boolean;
		onopenchange: (open: boolean) => void;
		onrevoked: (result: RevokeCertificateResult) => void;
	} = $props();

	let reason = $state('');
	let createReplacementCandidate = $state(false);
	let busy = $state(false);
	let error = $state('');
	const allowed = $derived(
		canRevoke && certificate.status === 'issued' && certificate.capabilities.canRevoke === true
	);
	const canSubmit = $derived(
		allowed && reason.trim().length >= 1 && reason.trim().length <= 500 && !busy
	);

	function reset() {
		reason = '';
		createReplacementCandidate = false;
		error = '';
	}

	function changeOpen(nextOpen: boolean) {
		if (busy) return;
		if (!nextOpen) reset();
		onopenchange(nextOpen);
	}

	async function revoke() {
		if (!canSubmit) return;
		busy = true;
		error = '';
		try {
			const result = await revokeIssuedCertificate(certificate.id, {
				reason: reason.trim(),
				createReplacementCandidate
			});
			onrevoked(result);
			toast.success(`เพิกถอน ${certificate.certificateNumber} แล้ว`);
			reset();
			onopenchange(false);
		} catch (revokeError) {
			error = revokeError instanceof Error ? revokeError.message : 'เพิกถอนเกียรติบัตรไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>เพิกถอนเกียรติบัตร</Dialog.Title>
			<Dialog.Description>
				{certificate.certificateNumber} · {certificate.title ?? ''}{certificate.firstName}
				{certificate.lastName}
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4">
			<div class="flex gap-3 rounded-xl border border-red-200 bg-red-50 p-4 text-red-950">
				<ShieldAlert class="mt-0.5 size-5 shrink-0" />
				<div>
					<p class="font-medium">เลขเดิมจะคงอยู่ในทะเบียน</p>
					<p class="mt-1 text-sm text-red-800">
						ใบนี้จะตรวจสอบได้เฉพาะสถานะเพิกถอนและดาวน์โหลดไม่ได้ ระบบจะไม่นำเลขเดิมกลับมาใช้อีก
					</p>
				</div>
			</div>

			<label class="block space-y-1.5">
				<span class="text-sm font-medium">เหตุผลการเพิกถอน</span>
				<Textarea
					bind:value={reason}
					rows={4}
					maxlength={500}
					placeholder="ระบุสาเหตุภายในที่ตรวจสอบย้อนหลังได้ โดยไม่ใส่ข้อมูลอ่อนไหว"
				/>
				<span class="block text-right text-xs text-muted-foreground">
					{reason.length.toLocaleString('th-TH')}/500
				</span>
			</label>

			<label class="flex items-start gap-3 rounded-xl border p-4">
				<input
					type="checkbox"
					class="mt-0.5 size-4 rounded border-input accent-primary"
					bind:checked={createReplacementCandidate}
				/>
				<FilePlus2 class="mt-0.5 size-4 shrink-0 text-primary" />
				<span>
					<span class="block text-sm font-medium">สร้างรายการทดแทนให้แก้ไขและส่งออกใหม่</span>
					<span class="mt-1 block text-xs text-muted-foreground">
						ระบบคัดลอกข้อมูลเดิมเป็นรายการเตรียมออก เลขใหม่จะเกิดหลังแก้และส่งคำขอรอบใหม่
					</span>
				</span>
			</label>

			<div class="flex items-start gap-2 text-xs text-muted-foreground">
				<AlertTriangle class="mt-0.5 size-4 shrink-0" />
				<span>การเพิกถอนย้อนกลับไม่ได้ โปรดตรวจเลขและชื่อผู้รับก่อนยืนยัน</span>
			</div>

			{#if error}
				<div
					role="alert"
					class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive"
				>
					{error}
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}>ยกเลิก</Button>
			<LoadingButton
				loading={busy}
				loadingLabel="กำลังเพิกถอน..."
				disabled={!canSubmit}
				variant="destructive"
				onclick={revoke}
			>
				<ShieldAlert class="size-4" /> ยืนยันเพิกถอน
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
