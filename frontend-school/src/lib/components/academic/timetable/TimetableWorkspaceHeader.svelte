<script lang="ts">
	import type { TimetableVersion } from '$lib/api/timetable';
	import { Badge } from '$lib/components/ui/badge';
	import { CalendarRange, Check, CloudCog, LoaderCircle, RefreshCw } from 'lucide-svelte';

	import type { TimetableBoardView } from '$lib/academic/timetable/board-state';
	import TimetableViewSelector from './TimetableViewSelector.svelte';

	let {
		version,
		view,
		isSaving = false,
		isRefreshing = false,
		onViewChange
	}: {
		version: TimetableVersion;
		view: TimetableBoardView;
		isSaving?: boolean;
		isRefreshing?: boolean;
		onViewChange: (view: TimetableBoardView) => void;
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

<header class="overflow-hidden rounded-xl border bg-background shadow-sm">
	<div class="h-1 bg-primary"></div>
	<div class="flex flex-col gap-4 p-4 lg:flex-row lg:items-center lg:justify-between">
		<div class="min-w-0 space-y-2">
			<div class="flex flex-wrap items-center gap-2">
				<h2 class="text-lg font-semibold tracking-tight">รุ่นตารางสอนที่เลือก</h2>
				<Badge variant={version.status === 'draft' ? 'secondary' : 'outline'}>
					{version.status === 'draft' ? 'แบบร่าง · แก้ไขได้' : 'เผยแพร่แล้ว · อ่านอย่างเดียว'}
				</Badge>
			</div>
			<div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-muted-foreground">
				<span class="inline-flex items-center gap-1.5">
					<CalendarRange class="size-4" />
					เริ่มใช้ {thaiDate(version.effectiveFrom)} – {thaiDate(version.effectiveUntil)}
				</span>
				<span class="inline-flex items-center gap-1.5" aria-live="polite">
					{#if isSaving}
						<LoaderCircle class="size-4 animate-spin" /> กำลังบันทึก
					{:else if isRefreshing}
						<RefreshCw class="size-4 animate-spin" /> กำลังโหลดข้อมูลล่าสุด
					{:else if version.status === 'draft'}
						<Check class="size-4 text-emerald-600" /> พร้อมจัดตาราง
					{:else}
						<CloudCog class="size-4" /> รุ่นที่ใช้อ้างอิง
					{/if}
				</span>
			</div>
		</div>
		<TimetableViewSelector value={view} {onViewChange} disabled={isSaving} />
	</div>
</header>
