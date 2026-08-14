<script lang="ts">
	import { resolve } from '$app/paths';
	import type { CertificateTemplateDetail } from '$lib/api/certificates';
	import { describePaper } from '$lib/certificates/paper';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import {
		ChevronDown,
		FileBadge2,
		FileCheck2,
		FileWarning,
		Pencil,
		QrCode,
		Settings2,
		Trash2,
		UsersRound
	} from 'lucide-svelte';
	import CertificateAssetManager from './CertificateAssetManager.svelte';
	import CertificateBackgroundUpload from './CertificateBackgroundUpload.svelte';
	import { toast } from 'svelte-sonner';

	let {
		campaignId,
		templates,
		onpatched,
		onedit,
		ondelete,
		oncreate,
		onpendingchange
	}: {
		campaignId: string;
		templates: CertificateTemplateDetail[];
		onpatched: (template: CertificateTemplateDetail) => void;
		onedit: (template: CertificateTemplateDetail) => void;
		ondelete: (template: CertificateTemplateDetail) => void;
		oncreate?: () => void;
		onpendingchange: (pending: boolean) => void;
	} = $props();

	let expandedTemplateId = $state<string | null>(null);
	let pendingUploadKeys = $state<string[]>([]);

	const recipientLabels: Record<
		CertificateTemplateDetail['allowedRecipientTypes'][number],
		string
	> = {
		student: 'นักเรียน',
		staff: 'บุคลากร',
		external: 'บุคคลภายนอก'
	};

	function paperDescription(template: CertificateTemplateDetail): string {
		const geometry = template.pageGeometry;
		if (!geometry) return 'ยังไม่ทราบขนาดกระดาษ';
		return describePaper({
			widthPoints: geometry.cropBox.widthPoints,
			heightPoints: geometry.cropBox.heightPoints,
			rotation: geometry.rotation
		});
	}

	function paperAspect(template: CertificateTemplateDetail): number {
		const geometry = template.pageGeometry;
		if (!geometry || geometry.displayedHeightPoints <= 0) return 1.414;
		return Math.min(
			2,
			Math.max(0.5, geometry.displayedWidthPoints / geometry.displayedHeightPoints)
		);
	}

	function toggleFiles(templateId: string) {
		if (expandedTemplateId && hasPendingUpload(expandedTemplateId)) {
			toast.error('แนบหรือลบไฟล์ชั่วคราวให้เสร็จก่อนปิดส่วนจัดการไฟล์');
			return;
		}
		expandedTemplateId = expandedTemplateId === templateId ? null : templateId;
	}

	function hasPendingUpload(templateId: string): boolean {
		return pendingUploadKeys.some((key) => key.startsWith(`${templateId}:`));
	}

	function setPendingUpload(templateId: string, source: 'background' | 'assets', pending: boolean) {
		const key = `${templateId}:${source}`;
		pendingUploadKeys = pending
			? Array.from(new Set([...pendingUploadKeys, key]))
			: pendingUploadKeys.filter((candidate) => candidate !== key);
		onpendingchange(pendingUploadKeys.length > 0);
	}
</script>

{#if templates.length === 0}
	<div class="rounded-2xl border border-dashed bg-muted/15 px-6 py-14 text-center">
		<div class="mx-auto grid size-14 place-items-center rounded-2xl border bg-background shadow-sm">
			<FileBadge2 class="size-7 text-primary" />
		</div>
		<h2 class="mt-4 text-lg font-semibold">ยังไม่มีแบบเกียรติบัตร</h2>
		<p class="mx-auto mt-2 max-w-md text-sm leading-relaxed text-muted-foreground">
			เริ่มจากตั้งชื่อ เลือกประเภทผู้รับ และแนบ PDF พื้นหลังหนึ่งหน้า
			ระบบจะอ่านขนาดกระดาษให้อัตโนมัติ
		</p>
		{#if oncreate}
			<Button class="mt-5" onclick={oncreate}>สร้างแบบแรก</Button>
		{/if}
	</div>
{:else}
	<div class="grid items-start gap-5 xl:grid-cols-2">
		{#each templates as template (template.id)}
			<Card.Root class="relative overflow-hidden py-0">
				<div
					class={[
						'absolute inset-y-0 left-0 w-1',
						template.isActive
							? template.isReady
								? 'bg-emerald-500'
								: 'bg-amber-400'
							: 'bg-muted-foreground/35'
					]}
				></div>
				<Card.Content class="p-5 pl-6">
					<div class="grid gap-5 sm:grid-cols-[8.5rem_minmax(0,1fr)]">
						<div class="rounded-xl border bg-muted/25 p-3">
							<div
								class="relative mx-auto grid max-h-36 min-h-24 w-full place-items-center overflow-hidden rounded-sm border border-foreground/15 bg-white shadow-sm"
								style:aspect-ratio={paperAspect(template)}
							>
								<div class="absolute inset-x-[12%] top-[18%] space-y-1.5" aria-hidden="true">
									<div class="mx-auto h-1.5 w-2/5 rounded-full bg-slate-300"></div>
									<div class="mx-auto h-1 w-4/5 rounded-full bg-slate-200"></div>
									<div class="mx-auto mt-3 h-2 w-3/5 rounded-full bg-slate-400"></div>
									<div class="mx-auto h-1 w-2/3 rounded-full bg-slate-200"></div>
								</div>
								<QrCode class="absolute bottom-[10%] right-[8%] size-[16%] text-slate-400" />
								{#if !template.backgroundFileId}
									<div
										class="absolute inset-0 grid place-items-center bg-amber-50/90 p-2 text-center text-[0.65rem] font-medium text-amber-800"
									>
										ยังไม่มี PDF
									</div>
								{/if}
							</div>
							<p class="mt-2 text-center text-xs font-medium text-muted-foreground">
								{paperDescription(template)}
							</p>
						</div>

						<div class="min-w-0">
							<div class="flex flex-wrap items-start justify-between gap-3">
								<div class="min-w-0">
									<h2 class="truncate text-base font-semibold">{template.name}</h2>
									<p class="mt-1 text-xs text-muted-foreground">
										{template.layout.elements.length} องค์ประกอบ · {template.assets.length} ไฟล์แนบ
									</p>
								</div>
								<Badge variant={template.isActive ? 'secondary' : 'outline'}>
									{template.isActive ? 'เปิดใช้' : 'ปิดใช้'}
								</Badge>
							</div>

							<div class="mt-4 space-y-3 text-sm">
								<div class="flex items-start gap-2">
									<UsersRound class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
									<div class="flex flex-wrap gap-1.5">
										{#each template.allowedRecipientTypes as recipientType (recipientType)}
											<Badge variant="outline">{recipientLabels[recipientType]}</Badge>
										{/each}
									</div>
								</div>
								<div
									class={[
										'flex items-center gap-2 text-xs font-medium',
										template.isReady ? 'text-emerald-700' : 'text-amber-700'
									]}
								>
									{#if template.isReady}
										<FileCheck2 class="size-4" /> พร้อมออกแบบและพรีวิว
									{:else}
										<FileWarning class="size-4" />
										{template.backgroundFileId
											? 'ไฟล์หรือองค์ประกอบยังไม่พร้อม'
											: 'รอแนบ PDF พื้นหลัง'}
									{/if}
								</div>
							</div>

							<div class="mt-5 flex flex-wrap gap-2">
								<Button
									size="sm"
									href={resolve(
										`/staff/certificates/${campaignId}/templates/${template.id}/editor` as '/staff/certificates'
									)}
									disabled={!template.capabilities.canUpdate ||
										!template.isReady ||
										hasPendingUpload(template.id)}
								>
									<Pencil class="size-4" /> เปิด editor
								</Button>
								<Button
									size="sm"
									variant="outline"
									onclick={() => toggleFiles(template.id)}
									aria-expanded={expandedTemplateId === template.id}
								>
									<Settings2 class="size-4" /> จัดการไฟล์
									<ChevronDown
										class={`size-3.5 transition-transform ${expandedTemplateId === template.id ? 'rotate-180' : ''}`}
									/>
								</Button>
								{#if template.capabilities.canUpdate}
									<Button
										size="sm"
										variant="ghost"
										onclick={() => onedit(template)}
										disabled={hasPendingUpload(template.id)}
									>
										แก้ข้อมูล
									</Button>
								{/if}
								{#if template.capabilities.canDelete}
									<Button
										size="icon-sm"
										variant="ghost"
										onclick={() => ondelete(template)}
										disabled={hasPendingUpload(template.id)}
										aria-label={`ลบ ${template.name}`}
									>
										<Trash2 class="size-4" />
									</Button>
								{/if}
							</div>
						</div>
					</div>
				</Card.Content>

				{#if expandedTemplateId === template.id}
					<div class="border-t bg-muted/10 p-5 pl-6">
						<div class="space-y-6">
							<CertificateBackgroundUpload
								{template}
								{onpatched}
								onpendingchange={(pending) => setPendingUpload(template.id, 'background', pending)}
							/>
							<div class="border-t"></div>
							<CertificateAssetManager
								{template}
								{onpatched}
								onpendingchange={(pending) => setPendingUpload(template.id, 'assets', pending)}
							/>
						</div>
					</div>
				{/if}
			</Card.Root>
		{/each}
	</div>
{/if}
