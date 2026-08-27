<script lang="ts">
	import type { LearningGroup, RosterPreview } from '$lib/api/learning-delivery';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { AlertTriangle, CheckCircle2, RefreshCw, Send, UserRound } from 'lucide-svelte';

	let {
		group,
		preview,
		loading = false,
		stale = false,
		canManage = false,
		onRefresh,
		onApply,
		onPublish
	}: {
		group: LearningGroup;
		preview: RosterPreview | null;
		loading?: boolean;
		stale?: boolean;
		canManage?: boolean;
		onRefresh: () => Promise<void>;
		onApply: (sourceHash: string) => Promise<void>;
		onPublish: () => Promise<void>;
	} = $props();

	let errorMessage = $state('');

	function rowState(student: RosterPreview['students'][number]) {
		if (student.conflictReason) return { label: 'ต้องตรวจสอบ', variant: 'destructive' as const };
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

<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
	<header class="flex flex-wrap items-center justify-between gap-3 border-b bg-muted/25 p-4">
		<div>
			<h2 class="font-semibold">รายชื่อนักเรียน · {group.name}</h2>
			<p class="mt-1 text-sm text-muted-foreground">
				สร้างตัวอย่างจากห้องต้นทาง แล้วตรวจการเพิ่ม นำออก และรายการขัดแย้งก่อนยืนยัน
			</p>
		</div>
		<Button size="sm" variant="outline" disabled={loading} onclick={() => perform(onRefresh)}>
			<RefreshCw class={loading ? 'size-4 animate-spin' : 'size-4'} />
			{preview ? 'สร้างตัวอย่างใหม่' : 'ตรวจรายชื่อ'}
		</Button>
	</header>

	{#if stale}
		<div class="flex gap-2 border-b border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
			<AlertTriangle class="mt-0.5 size-4 shrink-0" />
			<p>
				ข้อมูลต้นทางเปลี่ยนไปแล้ว ตัวอย่างนี้ยังแสดงไว้เพื่อเปรียบเทียบ แต่ต้องสร้างใหม่ก่อนยืนยัน
			</p>
		</div>
	{/if}

	{#if preview}
		<div class="grid grid-cols-2 gap-2 border-b p-4 sm:grid-cols-4">
			<div
				class="rounded-xl bg-emerald-50 p-3 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-300"
			>
				<p class="text-xl font-semibold">{preview.added}</p>
				<p class="text-xs">เพิ่มเข้ากลุ่ม</p>
			</div>
			<div
				class="rounded-xl bg-amber-50 p-3 text-amber-700 dark:bg-amber-950/40 dark:text-amber-300"
			>
				<p class="text-xl font-semibold">{preview.removed}</p>
				<p class="text-xs">นำออกจากกลุ่ม</p>
			</div>
			<div class="rounded-xl bg-muted p-3">
				<p class="text-xl font-semibold">{preview.retained}</p>
				<p class="text-xs">คงเดิม</p>
			</div>
			<div class="rounded-xl bg-red-50 p-3 text-red-700 dark:bg-red-950/40 dark:text-red-300">
				<p class="text-xl font-semibold">{preview.conflicts}</p>
				<p class="text-xs">ต้องตรวจสอบ</p>
			</div>
		</div>

		<div class="max-h-[28rem] divide-y overflow-auto">
			{#each preview.students as student (student.studentAcademicYearId)}
				{@const state = rowState(student)}
				<div class="flex items-center justify-between gap-3 px-4 py-3 text-sm">
					<div class="flex min-w-0 items-start gap-3">
						<div class="rounded-lg bg-primary/10 p-2 text-primary">
							<UserRound class="size-4" />
						</div>
						<div class="min-w-0">
							<p class="truncate font-medium">{student.displayName}</p>
							<p class="text-xs text-muted-foreground">
								{student.studentCode ?? 'ยังไม่มีรหัสนักเรียน'} · {student.gradeLevelName}
								{#if student.homeroomName}
									· {student.homeroomName}{/if}
							</p>
							{#if student.conflictReason}
								<p class="mt-1 flex items-center gap-1 text-xs text-destructive">
									<AlertTriangle class="size-3" />
									{student.conflictReason}
								</p>
							{/if}
						</div>
					</div>
					<Badge variant={state.variant}>{state.label}</Badge>
				</div>
			{:else}
				<div class="p-8 text-center text-sm text-muted-foreground">
					ไม่พบนักเรียนจากห้องต้นทางที่เลือก
				</div>
			{/each}
		</div>

		<footer class="flex flex-wrap items-center justify-between gap-3 border-t p-4">
			<p class="text-xs text-muted-foreground">
				ยืนยันเป็นฉบับร่างก่อน แล้วจึงเผยแพร่ให้ระบบปลายทางใช้
			</p>
			<div class="flex flex-wrap gap-2">
				<Button
					variant="outline"
					disabled={!canManage || loading || stale || preview.conflicts > 0}
					onclick={() => perform(() => onApply(preview.sourceHash))}
				>
					<CheckCircle2 class="size-4" /> ยืนยันเป็นฉบับร่าง
				</Button>
				<Button
					disabled={!canManage || loading || stale || group.rosterStatus !== 'draft'}
					onclick={() => perform(onPublish)}
				>
					<Send class="size-4" /> เผยแพร่รายชื่อ
				</Button>
			</div>
		</footer>
	{:else}
		<div class="p-10 text-center">
			<div class="mx-auto mb-3 w-fit rounded-2xl bg-primary/10 p-3 text-primary">
				<UserRound class="size-5" />
			</div>
			<p class="font-medium">ยังไม่ได้สร้างตัวอย่างรายชื่อ</p>
			<p class="mt-1 text-sm text-muted-foreground">
				ตรวจว่ากำหนดห้องต้นทางถูกต้อง แล้วกด “ตรวจรายชื่อ” เพื่อดูผลก่อนเปลี่ยนข้อมูลจริง
			</p>
		</div>
	{/if}

	{#if errorMessage}
		<p role="alert" class="border-t bg-destructive/5 px-4 py-3 text-sm text-destructive">
			{errorMessage}
		</p>
	{/if}
</section>
