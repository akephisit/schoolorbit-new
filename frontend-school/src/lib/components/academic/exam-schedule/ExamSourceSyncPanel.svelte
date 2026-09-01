<script lang="ts">
	import type {
		ExamSourceChange,
		ExamSourcePreview,
		ExamSourceSyncItemResult
	} from '$lib/api/examSchedule';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import {
		CheckCircle2,
		CircleAlert,
		Clock3,
		ListRestart,
		RefreshCw,
		ShieldCheck
	} from 'lucide-svelte';

	let {
		preview,
		selectedSourceIds = [],
		results = [],
		readonly = false,
		syncing = false,
		onToggle,
		onSelectRecommended,
		onSync
	}: {
		preview: ExamSourcePreview;
		selectedSourceIds?: string[];
		results?: ExamSourceSyncItemResult[];
		readonly?: boolean;
		syncing?: boolean;
		onToggle?: (sourceId: string, checked: boolean) => void;
		onSelectRecommended?: () => void;
		onSync?: () => void;
	} = $props();

	const hasChanges = $derived(preview.changes.length > 0);
	const selectedSet = $derived(new Set(selectedSourceIds));
	const resultMap = $derived(new Map(results.map((result) => [result.sourceId, result])));

	function changeLabel(change: ExamSourceChange): string {
		if (change.changeKind === 'new') return 'รายการใหม่';
		if (change.changeKind === 'duration_changed') return 'เวลาเปลี่ยน';
		return 'ไม่ผ่านเงื่อนไขแล้ว';
	}

	function changeDescription(change: ExamSourceChange): string {
		if (change.changeKind === 'new') {
			return `เพิ่มเข้าสู่รอบสอบ ${change.currentDurationMinutes ?? '-'} นาที`;
		}
		if (change.changeKind === 'duration_changed') {
			return `${change.snapshotDurationMinutes ?? '-'} → ${change.currentDurationMinutes ?? '-'} นาที${change.scheduled ? ' · จัดลงตารางแล้ว ระบบจะตรวจการชนอีกครั้ง' : ''}`;
		}
		return 'เลือกเมื่อต้องการนำ snapshot นี้ออกจากรอบสอบ';
	}

	function changeBadgeClass(change: ExamSourceChange): string {
		if (change.changeKind === 'new') return 'border-sky-300 text-sky-800 dark:text-sky-300';
		if (change.changeKind === 'duration_changed') {
			return 'border-amber-300 text-amber-800 dark:text-amber-300';
		}
		return 'border-rose-300 text-rose-800 dark:text-rose-300';
	}
</script>

{#if !hasChanges}
	<div
		class="flex items-start justify-between gap-4 rounded-xl border border-emerald-200 bg-emerald-50/70 p-4 text-emerald-950 dark:border-emerald-900 dark:bg-emerald-950/20 dark:text-emerald-100"
	>
		<div class="flex items-start gap-3">
			<ShieldCheck class="mt-0.5 size-5 shrink-0" />
			<div>
				<p class="font-medium">รายการสอบตรงกับโครงสร้างคะแนนแล้ว</p>
				<p class="mt-1 text-sm opacity-80">
					ยังไม่มีรายวิชาใหม่ เวลาเปลี่ยน หรือ snapshot ที่ต้องนำออก
				</p>
			</div>
		</div>
		{#if readonly}<Badge variant="outline">เผยแพร่แล้ว</Badge>{/if}
	</div>
{:else}
	<section class="overflow-hidden rounded-xl border bg-card shadow-sm">
		<div class="flex flex-wrap items-start justify-between gap-4 border-b px-5 py-4">
			<div>
				<div class="flex items-center gap-2">
					<ListRestart class="size-5 text-primary" />
					<h2 class="font-semibold">ตรวจการเปลี่ยนแปลงรายการสอบ</h2>
				</div>
				<p class="mt-1 text-sm text-muted-foreground">
					รายการในรอบสอบเป็น snapshot จึงเปลี่ยนเฉพาะรายการที่เลือกด้านล่าง
				</p>
			</div>
			<div class="flex flex-wrap items-center gap-2">
				<Badge variant="outline" class="border-sky-300 text-sky-800 dark:text-sky-300"
					>ใหม่ {preview.newCount}</Badge
				>
				<Badge variant="outline" class="border-amber-300 text-amber-800 dark:text-amber-300"
					>เวลาเปลี่ยน {preview.durationChangedCount}</Badge
				>
				<Badge variant="outline" class="border-rose-300 text-rose-800 dark:text-rose-300"
					>ไม่เข้าเงื่อนไข {preview.noLongerEligibleCount}</Badge
				>
			</div>
		</div>

		<div class="max-h-72 divide-y overflow-y-auto">
			{#each preview.changes as change (change.sourceId)}
				{@const result = resultMap.get(change.sourceId)}
				<label class="flex cursor-pointer items-start gap-3 px-5 py-3 hover:bg-muted/35">
					<Checkbox
						class="mt-1"
						checked={selectedSet.has(change.sourceId)}
						disabled={readonly || syncing}
						onCheckedChange={(checked) => onToggle?.(change.sourceId, checked === true)}
					/>
					<div class="min-w-0 flex-1">
						<div class="flex flex-wrap items-center gap-2">
							<span class="font-medium">{change.subjectCode} · {change.subjectName}</span>
							<Badge variant="outline" class={changeBadgeClass(change)}>{changeLabel(change)}</Badge
							>
							{#if result?.status === 'applied'}
								<span
									class="inline-flex items-center gap-1 text-xs text-emerald-700 dark:text-emerald-300"
									><CheckCircle2 class="size-3" /> อัปเดตแล้ว</span
								>
							{:else if result?.status === 'conflict'}
								<span class="inline-flex items-center gap-1 text-xs text-destructive"
									><CircleAlert class="size-3" /> ยังไม่อัปเดต</span
								>
							{/if}
						</div>
						<p class="mt-1 text-xs text-muted-foreground">
							{change.homeroomName} · {changeDescription(change)}
						</p>
						{#if result?.message}<p class="mt-1 text-xs text-destructive">{result.message}</p>{/if}
					</div>
				</label>
			{/each}
		</div>

		<div class="flex flex-wrap items-center justify-between gap-3 border-t bg-muted/20 px-5 py-3">
			<div class="flex items-center gap-2 text-xs text-muted-foreground">
				<Clock3 class="size-4" />
				{#if readonly}รอบที่เผยแพร่แล้วเก็บ snapshot เดิมและแก้รายการไม่ได้{:else}เลือกรายการที่ต้องการซิงก์
					{selectedSourceIds.length} รายการ{/if}
			</div>
			{#if !readonly}
				<div class="flex items-center gap-2">
					<Button variant="outline" size="sm" disabled={syncing} onclick={onSelectRecommended}
						>เลือกเฉพาะเพิ่ม/อัปเดต</Button
					>
					<LoadingButton
						size="sm"
						loading={syncing}
						loadingLabel="กำลังซิงก์..."
						disabled={selectedSourceIds.length === 0}
						onclick={onSync}
					>
						<RefreshCw class="size-4" /> ซิงก์รายการที่เลือก
					</LoadingButton>
				</div>
			{/if}
		</div>
	</section>
{/if}
