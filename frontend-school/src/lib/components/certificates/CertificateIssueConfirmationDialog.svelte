<script lang="ts">
	import type {
		CertificateIssueRequestDetail,
		CertificateIssueRequestItem
	} from '$lib/api/certificates';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { AlertTriangle, Eye, FileBadge2, Hash, ShieldCheck, UsersRound } from 'lucide-svelte';
	import { SvelteMap } from 'svelte/reactivity';

	let {
		open,
		request,
		busy = false,
		error = '',
		onopenchange,
		onconfirm,
		onpreview
	}: {
		open: boolean;
		request: CertificateIssueRequestDetail;
		busy?: boolean;
		error?: string;
		onopenchange: (open: boolean) => void;
		onconfirm: () => Promise<void>;
		onpreview: (item: CertificateIssueRequestItem) => void;
	} = $props();

	type TemplateGroup = {
		id: string;
		name: string;
		items: CertificateIssueRequestItem[];
	};

	const templateGroups = $derived.by(() => {
		const groups = new SvelteMap<string, TemplateGroup>();
		for (const item of request.items) {
			const id = item.templateId ?? 'missing';
			const group = groups.get(id) ?? {
				id,
				name: item.templateName ?? 'ไม่พบแบบเกียรติบัตร',
				items: []
			};
			group.items.push(item);
			groups.set(id, group);
		}
		return [...groups.values()].sort((left, right) => left.name.localeCompare(right.name, 'th'));
	});

	const recipientCounts = $derived.by(() => ({
		student: request.items.filter((item) => item.recipientType === 'student').length,
		staff: request.items.filter((item) => item.recipientType === 'staff').length,
		external: request.items.filter((item) => item.recipientType === 'external').length
	}));

	function changeOpen(nextOpen: boolean) {
		if (!busy) onopenchange(nextOpen);
	}
</script>

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content class="max-h-[92vh] overflow-y-auto sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>ยืนยันออกเลขเกียรติบัตร</Dialog.Title>
			<Dialog.Description>
				ตรวจผลประเมินล่าสุดและแบบที่ใช้ใน {request.campaignName} ก่อนออกเลขจริง
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4">
			<div class="grid gap-px overflow-hidden rounded-xl border bg-border sm:grid-cols-4">
				<div class="bg-card p-4">
					<p class="text-xs text-muted-foreground">พร้อมออก</p>
					<p class="mt-1 text-2xl font-semibold tabular-nums">
						{request.readyCount.toLocaleString('th-TH')}
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

			<div class="flex gap-3 rounded-xl border border-blue-200 bg-blue-50 p-4 text-blue-950">
				<Hash class="mt-0.5 size-5 shrink-0" />
				<div>
					<p class="font-medium">ยังไม่มีเลขเกียรติบัตรถูกจอง</p>
					<p class="mt-1 text-sm text-blue-800">
						ระบบจะตรวจข้อมูลและไฟล์ที่ใช้อีกครั้งภายในคำสั่งเดียว
						แล้วจึงจัดเลขเรียงต่อเนื่องเมื่อยืนยันสำเร็จเท่านั้น
					</p>
				</div>
			</div>

			{#if request.reviewCount > 0 || request.invalidCount > 0}
				<div class="flex gap-3 rounded-xl border border-amber-200 bg-amber-50 p-4 text-amber-950">
					<AlertTriangle class="mt-0.5 size-5 shrink-0" />
					<div>
						<p class="font-medium">ผลประเมินปัจจุบันยังมีรายการที่ต้องตรวจ</p>
						<p class="mt-1 text-sm text-amber-800">
							ต้องตรวจสอบ {request.reviewCount.toLocaleString('th-TH')} รายการ · ไม่ถูกต้อง
							{request.invalidCount.toLocaleString('th-TH')} รายการ ระบบจะไม่จัดเลขหากยังไม่พร้อม
						</p>
					</div>
				</div>
			{/if}

			<section class="overflow-hidden rounded-xl border" aria-label="สรุปแบบที่จะออกเลข">
				<div class="flex items-center justify-between gap-3 border-b bg-muted/35 px-4 py-3">
					<div class="flex items-center gap-2">
						<FileBadge2 class="size-4 text-primary" />
						<h3 class="text-sm font-semibold">แบบเกียรติบัตรในคำขอ</h3>
					</div>
					<Badge variant="secondary">{templateGroups.length.toLocaleString('th-TH')} แบบ</Badge>
				</div>
				<div class="divide-y">
					{#each templateGroups as group (group.id)}
						<div class="flex flex-wrap items-center justify-between gap-3 px-4 py-3">
							<div class="min-w-0">
								<p class="truncate font-medium">{group.name}</p>
								<p class="mt-0.5 text-xs text-muted-foreground">
									{group.items.length.toLocaleString('th-TH')} ใบ
								</p>
							</div>
							<Button
								size="sm"
								variant="outline"
								disabled={!group.items[0]?.templateId || busy}
								onclick={() => group.items[0] && onpreview(group.items[0])}
							>
								<Eye class="size-4" /> ดูตัวอย่างแบบนี้
							</Button>
						</div>
					{/each}
				</div>
			</section>

			<div
				class="flex items-start gap-2 rounded-lg bg-muted/60 px-4 py-3 text-xs text-muted-foreground"
			>
				<UsersRound class="mt-0.5 size-4 shrink-0" />
				<span>
					ชื่อผู้รับ รายการ/รางวัล วันที่ออก และค่าคอลัมน์เสริมจะถูกเก็บเป็นข้อมูลของใบหลังออกเลข
				</span>
			</div>

			{#if error}
				<div
					role="alert"
					class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive"
				>
					<p class="font-medium">ยังยืนยันผลการออกเลขไม่ได้</p>
					<p class="mt-1">{error}</p>
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => onopenchange(false)}>ยกเลิก</Button>
			<LoadingButton loading={busy} onclick={onconfirm}>
				<ShieldCheck class="size-4" />
				{error ? 'ลองออกอีกครั้ง' : `ยืนยันออกเลข ${request.itemCount.toLocaleString('th-TH')} ใบ`}
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
