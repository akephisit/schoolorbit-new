<script lang="ts">
	import type { ElementAlignment } from '$lib/certificates/editor-state';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import {
		AlignCenterHorizontal,
		AlignCenterVertical,
		AlignStartHorizontal,
		AlignStartVertical,
		Braces,
		Copy,
		Eye,
		FileInput,
		Maximize2,
		Minus,
		Plus,
		QrCode,
		Save,
		ShieldCheck,
		Trash2
	} from 'lucide-svelte';

	let {
		templateName,
		dirty,
		saving,
		canSave,
		zoom,
		showSafeArea,
		snapToGuides,
		selectionCount,
		previewing,
		canPreviewCandidate = false,
		editingDisabled = false,
		backgroundDisabled = false,
		onsave,
		onzoom,
		onfit,
		onsafetoggle,
		onsnapchange,
		onpreview,
		onalign,
		onduplicate,
		ondelete,
		onbackground
	}: {
		templateName: string;
		dirty: boolean;
		saving: boolean;
		canSave: boolean;
		zoom: number;
		showSafeArea: boolean;
		snapToGuides: boolean;
		selectionCount: number;
		previewing: 'short' | 'normal' | 'long' | 'candidate' | null;
		canPreviewCandidate?: boolean;
		editingDisabled?: boolean;
		backgroundDisabled?: boolean;
		onsave: () => void;
		onzoom: (direction: 'in' | 'out') => void;
		onfit: () => void;
		onsafetoggle: () => void;
		onsnapchange: (enabled: boolean) => void;
		onpreview: (kind: 'short' | 'normal' | 'long' | 'candidate') => void;
		onalign: (alignment: ElementAlignment) => void;
		onduplicate: () => void;
		ondelete: () => void;
		onbackground: () => void;
	} = $props();

	const alignmentActions: Array<{
		value: ElementAlignment;
		label: string;
		icon: typeof AlignStartVertical;
	}> = [
		{ value: 'left', label: 'ชิดซ้าย', icon: AlignStartVertical },
		{ value: 'center', label: 'กึ่งกลางแนวนอน', icon: AlignCenterVertical },
		{ value: 'top', label: 'ชิดบน', icon: AlignStartHorizontal },
		{ value: 'middle', label: 'กึ่งกลางแนวตั้ง', icon: AlignCenterHorizontal }
	];
</script>

<header class="border-b bg-background/95 shadow-sm backdrop-blur" aria-label="แถบเครื่องมือ editor">
	<div class="flex min-h-14 items-center gap-2 overflow-x-auto px-3 py-2">
		<div class="mr-2 min-w-36 max-w-64 shrink-0">
			<p class="truncate text-sm font-semibold">{templateName}</p>
			<p class={['text-[0.68rem]', dirty ? 'text-amber-700' : 'text-muted-foreground']}>
				{dirty ? 'มีการแก้ไขที่ยังไม่บันทึก' : 'บันทึกล่าสุดแล้ว'}
			</p>
		</div>

		<LoadingButton
			size="sm"
			loading={saving}
			loadingLabel="กำลังบันทึก..."
			disabled={!canSave || !dirty}
			onclick={onsave}
		>
			<Save class="size-4" /> บันทึก
		</LoadingButton>

		<div class="mx-1 h-7 w-px shrink-0 bg-border"></div>

		<div class="flex shrink-0 items-center rounded-md border bg-muted/20 p-0.5">
			<Button size="icon-sm" variant="ghost" onclick={() => onzoom('out')} aria-label="ย่อมุมมอง">
				<Minus class="size-3.5" />
			</Button>
			<span class="w-12 text-center text-[0.7rem] font-medium tabular-nums">
				{Math.round(zoom * 100)}%
			</span>
			<Button size="icon-sm" variant="ghost" onclick={() => onzoom('in')} aria-label="ขยายมุมมอง">
				<Plus class="size-3.5" />
			</Button>
			<Button size="icon-sm" variant="ghost" onclick={onfit} aria-label="พอดีกับพื้นที่ทำงาน">
				<Maximize2 class="size-3.5" />
			</Button>
		</div>

		<Button
			size="sm"
			variant={showSafeArea ? 'secondary' : 'ghost'}
			disabled={editingDisabled}
			onclick={onsafetoggle}
			aria-pressed={showSafeArea}
		>
			<ShieldCheck class="size-4" /> พื้นที่ปลอดภัย
		</Button>
		<label class="flex shrink-0 items-center gap-2 px-2 text-xs text-muted-foreground">
			<input
				type="checkbox"
				checked={snapToGuides}
				onchange={(event) => onsnapchange(event.currentTarget.checked)}
				class="size-3.5 rounded border"
			/>
			ดูดแนว
		</label>

		{#if selectionCount > 0}
			<div class="mx-1 h-7 w-px shrink-0 bg-border"></div>
			<div class="flex shrink-0 items-center gap-0.5">
				{#each alignmentActions as action (action.value)}
					<Button
						size="icon-sm"
						variant="ghost"
						disabled={editingDisabled || selectionCount < 2}
						onclick={() => onalign(action.value)}
						aria-label={action.label}
					>
						<action.icon class="size-3.5" />
					</Button>
				{/each}
				<Button
					size="icon-sm"
					variant="ghost"
					disabled={editingDisabled}
					onclick={onduplicate}
					aria-label="ทำสำเนา"
				>
					<Copy class="size-3.5" />
				</Button>
				<Button
					size="icon-sm"
					variant="ghost"
					disabled={editingDisabled}
					onclick={ondelete}
					aria-label="ลบองค์ประกอบ"
				>
					<Trash2 class="size-3.5" />
				</Button>
			</div>
		{/if}

		<div class="ml-auto flex shrink-0 items-center gap-1">
			<Button
				size="sm"
				variant="outline"
				disabled={editingDisabled || backgroundDisabled}
				onclick={onbackground}
				title={backgroundDisabled ? 'บันทึกการจัดวางก่อนเปลี่ยนพื้นหลัง' : undefined}
			>
				<FileInput class="size-4" /> เปลี่ยนพื้นหลัง
			</Button>
			<span class="ml-1 flex items-center gap-1 text-[0.68rem] font-medium text-muted-foreground">
				<Eye class="size-3.5" /> พรีวิว
			</span>
			<Button
				size="sm"
				variant="ghost"
				disabled={previewing !== null}
				onclick={() => onpreview('short')}
			>
				<Braces class="size-3.5" /> ชื่อสั้น
			</Button>
			<Button
				size="sm"
				variant="ghost"
				disabled={previewing !== null}
				onclick={() => onpreview('normal')}
			>
				ชื่อปกติ
			</Button>
			<Button
				size="sm"
				variant="ghost"
				disabled={previewing !== null}
				onclick={() => onpreview('long')}
			>
				ชื่อยาว
			</Button>
			<Button
				size="sm"
				variant="ghost"
				disabled={previewing !== null || !canPreviewCandidate}
				onclick={() => onpreview('candidate')}
				title={canPreviewCandidate ? undefined : 'ใช้ได้หลังนำเข้ารายชื่อผู้รับ'}
			>
				<QrCode class="size-3.5" /> ผู้รับจริง
			</Button>
		</div>
	</div>
</header>
