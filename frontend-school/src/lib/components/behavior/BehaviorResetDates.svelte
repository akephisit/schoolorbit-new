<script lang="ts">
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { formatCalendarDate } from '#lib/utils/calendar.js';
	let { id, label, dates = $bindable([]) }: { id: string; label: string; dates: string[] } = $props();
	let selected = $state('');
	function add() {
		if (!selected) return;
		dates = [...new Set([...dates, selected])].toSorted();
		selected = '';
	}
</script>
<div class="space-y-2">
	<Label for={id}>{label}</Label>
	<div class="flex flex-wrap gap-2"><DatePicker {id} bind:value={selected} ariaLabel={label} /><Button type="button" variant="outline" disabled={!selected} onclick={add}>เพิ่มวันที่</Button></div>
	<p class="text-xs text-muted-foreground">ระบุวันที่สิ้นสุดแต่ละรอบตามงานพฤติกรรม</p>
	<ul class="space-y-1">{#each dates as date (date)}<li class="flex items-center justify-between gap-3 rounded-lg border px-3 py-2 text-sm"><span>{formatCalendarDate(date)}</span><Button type="button" variant="ghost" size="sm" aria-label={`นำวันที่ ${date} ออก`} onclick={() => dates = dates.filter((value) => value !== date)}>นำออก</Button></li>{/each}</ul>
	{#if dates.length === 0}<p class="text-xs text-muted-foreground">ยังไม่ได้กำหนดวันรีเซ็ต ระบบจะไม่รีเซ็ตตามกำหนดวันที่จนกว่าจะเพิ่มวัน</p>{/if}
</div>
