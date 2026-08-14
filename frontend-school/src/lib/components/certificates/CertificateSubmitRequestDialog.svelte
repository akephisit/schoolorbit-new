<script lang="ts">
	import { resolve } from '$app/paths';
	import type { CertificateCandidateDetail } from '$lib/api/certificates';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { AlertTriangle, FileBadge2, Send, UsersRound } from 'lucide-svelte';
	import { SvelteMap } from 'svelte/reactivity';

	let {
		open,
		campaignId,
		campaignName,
		candidates,
		busy = false,
		error = '',
		lockedRequestId = null,
		onopenchange,
		onsubmit
	}: {
		open: boolean;
		campaignId: string;
		campaignName: string;
		candidates: CertificateCandidateDetail[];
		busy?: boolean;
		error?: string;
		lockedRequestId?: string | null;
		onopenchange: (open: boolean) => void;
		onsubmit: (candidateIds: string[]) => Promise<void>;
	} = $props();

	type TemplateGroup = {
		key: string;
		name: string;
		candidates: CertificateCandidateDetail[];
	};

	const readyCandidates = $derived(
		candidates.filter((candidate) => candidate.validationStatus === 'ready')
	);
	const allReady = $derived(candidates.length > 0 && readyCandidates.length === candidates.length);
	const templateGroups = $derived.by(() => {
		const groups = new SvelteMap<string, TemplateGroup>();
		for (const candidate of readyCandidates) {
			const key = candidate.templateId ?? 'missing';
			const current = groups.get(key) ?? {
				key,
				name: candidate.templateName ?? 'ยังไม่กำหนดแบบ',
				candidates: []
			};
			current.candidates.push(candidate);
			groups.set(key, current);
		}
		return [...groups.values()].sort((left, right) => left.name.localeCompare(right.name, 'th'));
	});
	const recipientCounts = $derived.by(() => ({
		student: readyCandidates.filter((candidate) => candidate.recipientType === 'student').length,
		staff: readyCandidates.filter((candidate) => candidate.recipientType === 'staff').length,
		external: readyCandidates.filter((candidate) => candidate.recipientType === 'external').length
	}));

	async function submit() {
		if (!allReady || busy) return;
		await onsubmit(readyCandidates.map((candidate) => candidate.id));
	}
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="max-h-[92vh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>ส่งคำขอออกเกียรติบัตร</Dialog.Title>
			<Dialog.Description>
				ตรวจจำนวนและแบบที่จะใช้ใน {campaignName} ก่อนส่งให้ผู้มีสิทธิ์ระดับโรงเรียนตรวจ
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4">
			<div
				class="grid grid-cols-2 gap-px overflow-hidden rounded-xl border bg-border sm:grid-cols-4"
			>
				<div class="bg-card p-4">
					<p class="text-xs text-muted-foreground">พร้อมส่ง</p>
					<p class="mt-1 text-2xl font-semibold tabular-nums">
						{readyCandidates.length.toLocaleString('th-TH')}
					</p>
				</div>
				<div class="bg-card p-4">
					<p class="text-xs text-muted-foreground">นักเรียน</p>
					<p class="mt-1 text-2xl font-semibold tabular-nums">
						{recipientCounts.student.toLocaleString('th-TH')}
					</p>
				</div>
				<div class="bg-card p-4">
					<p class="text-xs text-muted-foreground">บุคลากร</p>
					<p class="mt-1 text-2xl font-semibold tabular-nums">
						{recipientCounts.staff.toLocaleString('th-TH')}
					</p>
				</div>
				<div class="bg-card p-4">
					<p class="text-xs text-muted-foreground">บุคคลภายนอก</p>
					<p class="mt-1 text-2xl font-semibold tabular-nums">
						{recipientCounts.external.toLocaleString('th-TH')}
					</p>
				</div>
			</div>

			{#if !allReady}
				<div
					class="flex gap-3 rounded-lg border border-amber-200 bg-amber-50 p-4 text-sm text-amber-950"
				>
					<AlertTriangle class="mt-0.5 size-4 shrink-0" />
					<div>
						<p class="font-medium">เลือกเฉพาะรายการสถานะพร้อมออก</p>
						<p class="mt-1 text-amber-800">
							มี {(candidates.length - readyCandidates.length).toLocaleString('th-TH')}
							รายการที่ยังส่งไม่ได้ กรุณาปิดหน้าต่างแล้วตรวจรายการที่เลือก
						</p>
					</div>
				</div>
			{/if}

			<section class="overflow-hidden rounded-xl border" aria-label="สรุปตามแบบเกียรติบัตร">
				<div class="flex items-center justify-between gap-3 border-b bg-muted/35 px-4 py-3">
					<div class="flex items-center gap-2">
						<FileBadge2 class="size-4 text-primary" />
						<h3 class="text-sm font-semibold">แบบที่ใช้ในคำขอนี้</h3>
					</div>
					<span class="text-xs text-muted-foreground">
						{templateGroups.length.toLocaleString('th-TH')} แบบ
					</span>
				</div>
				<div class="divide-y">
					{#each templateGroups as group (group.key)}
						<div class="flex items-center justify-between gap-4 px-4 py-3">
							<div>
								<p class="font-medium">{group.name}</p>
								<p class="mt-0.5 text-xs text-muted-foreground">
									ตรวจความพร้อมซ้ำอีกครั้งเมื่อผู้ตรวจเริ่มดำเนินการ
								</p>
							</div>
							<Badge variant="secondary">
								{group.candidates.length.toLocaleString('th-TH')} รายการ
							</Badge>
						</div>
					{/each}
				</div>
			</section>

			{#if error}
				<div
					class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive"
				>
					<p>{error}</p>
					{#if lockedRequestId}
						<a
							href={resolve(
								`/staff/certificates/${campaignId}/requests#request-${lockedRequestId}` as '/staff/certificates/[campaignId]/requests'
							)}
							class="mt-2 inline-flex font-medium underline underline-offset-4"
						>
							เปิดคำขอที่ล็อกรายการนี้ในประวัติกิจกรรม
						</a>
					{/if}
				</div>
			{/if}

			<div class="flex items-start gap-2 rounded-lg bg-blue-50 px-4 py-3 text-xs text-blue-900">
				<UsersRound class="mt-0.5 size-4 shrink-0" />
				<span>เมื่อส่งแล้ว รายการที่เลือกและแบบที่เกี่ยวข้องจะถูกล็อกจนกว่าจะถอนหรือส่งกลับ</span>
			</div>
		</div>

		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => onopenchange(false)}>ยกเลิก</Button>
			<LoadingButton loading={busy} disabled={!allReady} onclick={submit}>
				<Send class="size-4" /> ยืนยันส่ง {readyCandidates.length.toLocaleString('th-TH')} รายการ
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
