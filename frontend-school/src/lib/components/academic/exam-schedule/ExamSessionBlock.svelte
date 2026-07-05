<script lang="ts">
	import type { ExamSession } from '$lib/api/examSchedule';

	let {
		session,
		leftPx,
		widthPx,
		readonly = false,
		onDragStart,
		onOpen
	}: {
		session: ExamSession;
		leftPx: number;
		widthPx: number;
		readonly?: boolean;
		onDragStart?: (event: DragEvent, session: ExamSession, dragOffsetPx: number) => void;
		onOpen?: (session: ExamSession) => void;
	} = $props();

	function subjectLabel(): string {
		return session.subjectNameTh || session.subjectNameEn || session.subjectCode || 'ไม่ระบุวิชา';
	}

	function timeLabel(): string {
		return `${session.startsAt.slice(0, 5)}-${session.endsAt.slice(0, 5)}`;
	}

	function handleDragStart(event: DragEvent) {
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
		class="session-block absolute top-1 cursor-grab rounded border border-primary/30 bg-primary/10 px-2 py-1 text-left text-primary shadow-sm"
		style:left={`${leftPx}px`}
		style:width={`${widthPx}px`}
		draggable={true}
		aria-label={`จัดเวลา ${subjectLabel()} ${timeLabel()}`}
		ondragstart={handleDragStart}
		onclick={() => onOpen?.(session)}
	>
		<div class="truncate text-xs font-semibold leading-tight">{subjectLabel()}</div>
		<div class="truncate font-mono text-[11px] leading-tight opacity-80">{timeLabel()}</div>
	</button>
{/if}

<style>
	.session-block {
		min-height: 2.25rem;
		overflow: hidden;
	}
</style>
