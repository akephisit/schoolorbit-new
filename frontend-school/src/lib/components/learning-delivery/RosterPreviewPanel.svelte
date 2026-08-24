<script lang="ts">
	import type { LearningGroup, RosterPreview } from '$lib/api/learning-delivery';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { AlertTriangle, CheckCircle2, RefreshCw, Send } from 'lucide-svelte';

	let {
		group,
		preview,
		loading = false,
		canManage = false,
		onRefresh,
		onApply,
		onPublish
	}: {
		group: LearningGroup;
		preview: RosterPreview | null;
		loading?: boolean;
		canManage?: boolean;
		onRefresh: () => Promise<void>;
		onApply: (sourceHash: string) => Promise<void>;
		onPublish: () => Promise<void>;
	} = $props();

	let errorMessage = $state('');

	function rowState(student: RosterPreview['students'][number]) {
		if (student.conflictReason) return { label: 'ขัดแย้ง', variant: 'destructive' as const };
		if (!student.currentlyActive && student.proposedActive)
			return { label: 'เพิ่ม', variant: 'default' as const };
		if (student.currentlyActive && !student.proposedActive)
			return { label: 'นำออก', variant: 'secondary' as const };
		return { label: 'คงเดิม', variant: 'outline' as const };
	}

	async function perform(action: () => Promise<void>) {
		errorMessage = '';
		try {
			await action();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ดำเนินการรายชื่อไม่สำเร็จ';
		}
	}
</script>

<section class="rounded-xl border bg-card">
	<header class="flex flex-wrap items-center justify-between gap-3 border-b px-5 py-4">
		<div>
			<h2 class="font-semibold">ตรวจรายชื่อ · {group.name}</h2>
			<p class="text-xs text-muted-foreground">
				เปรียบเทียบห้องต้นทางกับรายชื่อที่ใช้อยู่ก่อนยืนยัน
			</p>
		</div>
		<Button size="sm" variant="outline" disabled={loading} onclick={() => perform(onRefresh)}
			><RefreshCw class={loading ? 'size-4 animate-spin' : 'size-4'} /> สร้างตัวอย่างใหม่</Button
		>
	</header>
	{#if preview}
		<div class="grid grid-cols-2 gap-2 border-b p-4 sm:grid-cols-4">
			<div class="rounded-lg bg-emerald-50 p-3 text-emerald-700">
				<p class="text-xl font-semibold">{preview.added}</p>
				<p class="text-xs">เพิ่ม</p>
			</div>
			<div class="rounded-lg bg-amber-50 p-3 text-amber-700">
				<p class="text-xl font-semibold">{preview.removed}</p>
				<p class="text-xs">นำออก</p>
			</div>
			<div class="rounded-lg bg-muted p-3">
				<p class="text-xl font-semibold">{preview.retained}</p>
				<p class="text-xs">คงเดิม</p>
			</div>
			<div class="rounded-lg bg-red-50 p-3 text-red-700">
				<p class="text-xl font-semibold">{preview.conflicts}</p>
				<p class="text-xs">ขัดแย้ง</p>
			</div>
		</div>
		<div class="max-h-80 divide-y overflow-auto">
			{#each preview.students as student (student.studentAcademicYearId)}{@const state =
					rowState(student)}
				<div class="flex items-center justify-between gap-3 px-5 py-3 text-sm">
					<div>
						<p class="font-medium">{student.studentId}</p>
						<p class="text-xs text-muted-foreground">{student.studentAcademicYearId}</p>
						{#if student.conflictReason}<p
								class="mt-1 flex items-center gap-1 text-xs text-destructive"
							>
								<AlertTriangle class="size-3" />
								{student.conflictReason}
							</p>{/if}
					</div>
					<Badge variant={state.variant}>{state.label}</Badge>
				</div>{/each}
		</div>
		<footer class="flex flex-wrap justify-end gap-2 border-t p-4">
			<Button
				variant="outline"
				disabled={!canManage || loading || preview.conflicts > 0}
				onclick={() => perform(() => onApply(preview.sourceHash))}
				><CheckCircle2 class="size-4" /> ยืนยันรายชื่อ</Button
			><Button
				disabled={!canManage || loading || group.rosterStatus !== 'draft'}
				onclick={() => perform(onPublish)}><Send class="size-4" /> เผยแพร่รายชื่อ</Button
			>
		</footer>
	{:else}
		<div class="p-10 text-center text-sm text-muted-foreground">
			กด “สร้างตัวอย่างใหม่” เพื่อดูผลกระทบก่อนเปลี่ยนรายชื่อจริง
		</div>
	{/if}
	{#if errorMessage}<p role="alert" class="border-t px-5 py-3 text-sm text-destructive">
			{errorMessage}
		</p>{/if}
</section>
