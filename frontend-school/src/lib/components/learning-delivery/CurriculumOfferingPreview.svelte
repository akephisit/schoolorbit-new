<script lang="ts">
	import type { CurriculumOfferingPreview } from '$lib/api/learning-delivery';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Eye, RefreshCw, WandSparkles } from 'lucide-svelte';

	let {
		preview,
		canManage = false,
		onPreview,
		onApply
	}: {
		preview: CurriculumOfferingPreview | null;
		canManage?: boolean;
		onPreview: (draft: {
			studyProgramIds: string[];
			owningOrganizationUnitId: string;
		}) => Promise<void>;
		onApply: (
			sourceHash: string,
			studyProgramIds: string[],
			owningOrganizationUnitId: string
		) => Promise<void>;
	} = $props();

	let programIdsText = $state('');
	let owningOrganizationUnitId = $state('');
	let busy = $state(false);
	let errorMessage = $state('');
	const programIds = $derived(
		programIdsText
			.split(',')
			.map((id) => id.trim())
			.filter(Boolean)
	);

	async function buildPreview(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		errorMessage = '';
		try {
			await onPreview({ studyProgramIds: programIds, owningOrganizationUnitId });
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างตัวอย่างไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function apply() {
		if (!preview) return;
		busy = true;
		errorMessage = '';
		try {
			await onApply(preview.sourceHash, programIds, owningOrganizationUnitId);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'นำรายการมาใช้ไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<section class="rounded-xl border bg-card">
	<header class="border-b px-5 py-4">
		<div class="flex items-center gap-2">
			<WandSparkles class="size-5 text-primary" />
			<h2 class="font-semibold">สร้างชุดการเรียนจากหลักสูตร</h2>
		</div>
		<p class="mt-1 text-xs text-muted-foreground">
			ตรวจรายการที่จะสร้าง คงเดิม และรายการขัดแย้งก่อนนำมาใช้
		</p>
	</header>
	<form
		class="grid gap-3 border-b p-5 md:grid-cols-[1fr_1fr_auto] md:items-end"
		onsubmit={buildPreview}
	>
		<div class="space-y-1.5">
			<Label for="preview-programs">รหัสแผนการเรียน (คั่นด้วยจุลภาค)</Label><Input
				id="preview-programs"
				bind:value={programIdsText}
				required
			/>
		</div>
		<div class="space-y-1.5">
			<Label for="preview-owner">รหัสหน่วยงานเจ้าของ</Label><Input
				id="preview-owner"
				bind:value={owningOrganizationUnitId}
				required
			/>
		</div>
		<Button type="submit" variant="outline" disabled={busy}
			><Eye class="size-4" /> สร้างตัวอย่าง</Button
		>
	</form>
	{#if preview}
		<div class="max-h-96 divide-y overflow-auto">
			{#each preview.items as item (item.requirementId)}<div
					class="grid gap-2 px-5 py-3 text-sm sm:grid-cols-[auto_1fr_auto] sm:items-center"
				>
					<Badge
						variant={item.action === 'conflict'
							? 'destructive'
							: item.action === 'create'
								? 'default'
								: 'secondary'}>{item.action}</Badge
					>
					<div>
						<p class="font-medium">{item.code} · {item.name}</p>
						<p class="text-xs text-muted-foreground">
							{item.resourceKind} · {item.gradeLevelId}{item.conflictReason
								? ` · ${item.conflictReason}`
								: ''}
						</p>
					</div>
					<span class="tabular-nums text-muted-foreground">{item.credit ?? item.hours ?? '—'}</span>
				</div>{/each}
		</div>
		<footer class="flex justify-end border-t p-4">
			<Button
				disabled={!canManage || busy || preview.items.some((item) => item.action === 'conflict')}
				onclick={apply}><WandSparkles class="size-4" /> นำรายการที่ตรวจแล้วมาใช้</Button
			>
		</footer>
	{:else}<div class="flex items-center justify-center gap-2 p-8 text-sm text-muted-foreground">
			<RefreshCw class="size-4" /> ยังไม่มีตัวอย่าง
		</div>{/if}
	{#if errorMessage}<p role="alert" class="border-t px-5 py-3 text-sm text-destructive">
			{errorMessage}
		</p>{/if}
</section>
