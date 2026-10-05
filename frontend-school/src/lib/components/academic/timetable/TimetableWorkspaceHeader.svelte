<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { TimetableVersion } from '#lib/api/timetable.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { CalendarRange, Check, CloudCog, LoaderCircle, RefreshCw } from '@lucide/svelte';

	import type { TimetablePageView } from '#lib/academic/timetable/board-state.js';
	import TimetableViewSelector from './TimetableViewSelector.svelte';

	let {
		version,
		view,
		isSaving = false,
		editing = false,
		isRefreshing = false,
		controls,
		onViewChange
	}: {
		version: TimetableVersion;
		view: TimetablePageView;
		isSaving?: boolean;
		editing?: boolean;
		isRefreshing?: boolean;
		controls: Snippet;
		onViewChange: (view: TimetablePageView) => void;
	} = $props();

	const thaiDate = (value: string | null) => {
		if (!value) return 'ต่อเนื่อง';
		return new Intl.DateTimeFormat('th-TH', {
			day: 'numeric',
			month: 'short',
			year: 'numeric'
		}).format(new Date(`${value}T00:00:00`));
	};
</script>

<header
	class="overflow-hidden rounded-xl border bg-background shadow-sm"
	aria-label="บริบทตารางสอน"
>
	<div class="h-1 bg-primary"></div>
	<div class="space-y-3 p-3 sm:p-4">
		<div class="min-w-0 space-y-2">
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="text-lg font-semibold tracking-tight">รุ่นตารางสอนที่เลือก</h2>
				<Badge variant={version.status === 'draft' ? 'secondary' : 'outline'}>
					{version.status === 'draft'
						? editing
							? 'แบบร่าง · กำลังแก้ไข'
							: 'แบบร่าง · โหมดดู'
						: 'เผยแพร่แล้ว · โหมดดู'}
				</Badge>
			</div>
			<div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-muted-foreground">
				<span class="inline-flex items-center gap-1.5">
					<CalendarRange class="size-4" />
					{#if version.effectiveFrom}เริ่มใช้ {thaiDate(version.effectiveFrom)} – {thaiDate(
							version.effectiveUntil
						)}{:else}เลือกวันที่เริ่มใช้ตอนเผยแพร่{/if}
				</span>
				<span class="inline-flex items-center gap-1.5" aria-live="polite">
					{#if isSaving}
						<LoaderCircle class="size-4 animate-spin" /> กำลังบันทึก
					{:else if isRefreshing}
						<RefreshCw class="size-4 animate-spin" /> กำลังโหลดข้อมูลล่าสุด
					{:else if version.status === 'draft'}
						<Check class="size-4 text-emerald-600" /> {editing ? 'บันทึกอัตโนมัติแล้ว' : 'โหมดดู'}
					{:else}
						<CloudCog class="size-4" /> รุ่นที่ใช้อ้างอิง
					{/if}
				</span>
			</div>
		</div>
		<div class="flex flex-col gap-3 xl:flex-row xl:items-end">
			<div class="min-w-0 flex-1">{@render controls()}</div>
			<TimetableViewSelector value={view} {onViewChange} disabled={isSaving} />
		</div>
	</div>
</header>
