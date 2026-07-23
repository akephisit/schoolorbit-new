<script lang="ts">
	import type { ExamSession } from '$lib/api/examSchedule';

	let {
		session,
		leftPx,
		widthPx,
		readonly = false,
		placing = false,
		removing = false,
		onDragStart,
		onOpen
	}: {
		session: ExamSession;
		leftPx: number;
		widthPx: number;
		readonly?: boolean;
		placing?: boolean;
		removing?: boolean;
		onDragStart?: (event: DragEvent, session: ExamSession, dragOffsetPx: number) => void;
		onOpen?: (session: ExamSession) => void;
	} = $props();

	const busy = $derived(placing || removing);
	const disabled = $derived(readonly || busy);

	function subjectLabel(): string {
		return session.subjectNameTh || session.subjectNameEn || session.subjectCode || 'ไม่ระบุวิชา';
	}

	function timeLabel(): string {
		return `${session.startsAt.slice(0, 5)}-${session.endsAt.slice(0, 5)}`;
	}

	function statusLabel(): string {
		if (removing) return 'กำลังเอาออก';
		if (placing) return 'กำลังบันทึก';
		return timeLabel();
	}

	function handleDragStart(event: DragEvent) {
		if (disabled) return;

		const target = event.currentTarget as HTMLElement;
		const dragOffsetPx = event.clientX - target.getBoundingClientRect().left;
		onDragStart?.(event, session, dragOffsetPx);
	}
</script>

{#if readonly}
	<div
		class="session-block absolute top-1 rounded border border-primary/30 bg-primary/10 px-2 py-1 text-primary shadow-sm"
		style:left={`${leftPx}px`}
		style:width={`${widthPx}px`}
	>
		<div class="truncate text-xs font-semibold leading-tight">{subjectLabel()}</div>
		<div class="truncate font-mono text-[11px] leading-tight opacity-80">{timeLabel()}</div>
	</div>
{:else}
	<button
		type="button"
		class={`session-block absolute top-1 rounded border px-2 py-1 text-left shadow-sm ${
			removing
				? 'border-destructive/40 bg-destructive/10 text-destructive'
				: 'border-primary/30 bg-primary/10 text-primary'
		} ${busy ? 'cursor-wait opacity-70' : 'cursor-grab'}`}
		style:left={`${leftPx}px`}
		style:width={`${widthPx}px`}
		draggable={!disabled}
		aria-busy={busy}
		aria-label={`จัดเวลา ${subjectLabel()} ${timeLabel()}`}
		{disabled}
		ondragstart={handleDragStart}
		onclick={() => onOpen?.(session)}
	>
		<div class="truncate text-xs font-semibold leading-tight">{subjectLabel()}</div>
		<div class="truncate font-mono text-[11px] leading-tight opacity-80">{statusLabel()}</div>
	</button>
{/if}

<style>
	.session-block {
		min-height: 2.25rem;
		overflow: hidden;
	}
</style>
