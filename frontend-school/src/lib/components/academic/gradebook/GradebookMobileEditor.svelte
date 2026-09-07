<script lang="ts" module>
	export interface MobileEditorOption {
		value: string;
		label: string;
	}
</script>

<script lang="ts">
	import type { GradebookSaveQueueSnapshot } from '$lib/academic/gradebook/save-queue';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Sheet from '$lib/components/ui/sheet';
	import { AlertCircle, Check, ChevronLeft, Loader2, RotateCcw } from 'lucide-svelte';

	let {
		open,
		mode,
		studentName,
		itemName,
		value,
		maxScore = null,
		options = [],
		saveStatus,
		onchange,
		onrequestclose,
		onretry
	}: {
		open: boolean;
		mode: 'score' | 'evaluation';
		studentName: string;
		itemName: string;
		value: string;
		maxScore?: string | null;
		options?: MobileEditorOption[];
		saveStatus: GradebookSaveQueueSnapshot;
		onchange: (value: string) => void;
		onrequestclose: () => void;
		onretry: () => void;
	} = $props();

	let selectedLabel = $derived(
		options.find((option) => option.value === value)?.label ?? 'ยังไม่ประเมิน'
	);
</script>

<Sheet.Root {open} onOpenChange={(next) => !next && onrequestclose()}>
	<Sheet.Content
		side="right"
		class="inset-0 h-[100dvh] w-full max-w-none gap-0 overflow-hidden p-0 sm:max-w-none"
		showCloseButton={false}
	>
		<Sheet.Header
			class="sticky top-0 z-20 flex-row items-center gap-3 border-b bg-background px-4 py-3 text-left"
		>
			<Button
				variant="ghost"
				size="icon"
				class="min-h-11 min-w-11"
				aria-label="ปิดหน้ากรอกคะแนน"
				onclick={onrequestclose}
			>
				<ChevronLeft class="size-5" />
			</Button>
			<div class="min-w-0 flex-1">
				<Sheet.Title class="truncate text-base">{studentName}</Sheet.Title>
				<Sheet.Description class="truncate">{itemName}</Sheet.Description>
			</div>
			<Button variant="outline" size="sm" onclick={onrequestclose}>ปิด</Button>
		</Sheet.Header>

		<div class="flex min-h-0 flex-1 flex-col justify-center overflow-y-auto px-5 py-8">
			<div class="mx-auto w-full max-w-sm space-y-3">
				<Label for="mobile-gradebook-value" class="text-base">
					{mode === 'score' ? 'คะแนนที่ได้' : 'ผลการประเมิน'}
				</Label>
				{#if mode === 'score'}
					<Input
						id="mobile-gradebook-value"
						class="h-16 text-center text-2xl font-semibold tabular-nums"
						inputmode="decimal"
						{value}
						placeholder="ยังไม่กรอก"
						onchange={(event) => onchange((event.currentTarget as HTMLInputElement).value)}
					/>
					<p class="text-center text-sm text-muted-foreground">
						คะแนนเต็ม {maxScore ?? '-'} · ลบค่าออกเพื่อกลับเป็นยังไม่กรอก
					</p>
				{:else}
					<Select.Root type="single" {value} onValueChange={onchange}>
						<Select.Trigger id="mobile-gradebook-value" class="h-14 w-full text-base">
							{selectedLabel}
						</Select.Trigger>
						<Select.Content>
							{#each options as option (option.value)}
								<Select.Item value={option.value}>{option.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				{/if}
			</div>
		</div>

		<footer
			class="sticky bottom-0 z-20 border-t bg-background px-4 py-3 pb-[max(0.75rem,env(safe-area-inset-bottom))]"
		>
			<div class="mx-auto flex max-w-sm items-center justify-between gap-3 text-sm">
				{#if saveStatus.state === 'saving'}
					<span class="flex items-center gap-2 text-muted-foreground"
						><Loader2 class="size-4 animate-spin" /> กำลังบันทึก</span
					>
				{:else if saveStatus.state === 'unsaved'}
					<span class="text-amber-700">มีข้อมูลรอบันทึก</span>
				{:else if saveStatus.state === 'failed'}
					<span class="flex items-center gap-2 text-destructive"
						><AlertCircle class="size-4" /> บันทึกไม่สำเร็จ</span
					>
					<Button variant="outline" size="sm" onclick={onretry}
						><RotateCcw class="size-4" /> ลองใหม่</Button
					>
				{:else}
					<span class="flex items-center gap-2 text-emerald-700"
						><Check class="size-4" /> บันทึกแล้ว</span
					>
				{/if}
			</div>
		</footer>
	</Sheet.Content>
</Sheet.Root>
