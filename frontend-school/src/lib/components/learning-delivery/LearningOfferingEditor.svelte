<script lang="ts">
	import type { LearningOffering } from '$lib/api/learning-delivery';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { BookOpen, CheckCircle2, Plus, Sparkles } from 'lucide-svelte';

	let {
		offerings,
		canManage = false,
		onSelect,
		onCreate,
		onPublish
	}: {
		offerings: LearningOffering[];
		canManage?: boolean;
		onSelect: (offering: LearningOffering) => void;
		onCreate: (draft: {
			kind: 'course' | 'activity';
			catalogVersionId: string;
			owningOrganizationUnitId: string;
			gradeLevelId: string;
			studyProgramId: string;
		}) => Promise<void>;
		onPublish: (offering: LearningOffering) => Promise<void>;
	} = $props();

	let draft = $state({
		kind: 'course' as 'course' | 'activity',
		catalogVersionId: '',
		owningOrganizationUnitId: '',
		gradeLevelId: '',
		studyProgramId: ''
	});
	let busy = $state(false);
	let errorMessage = $state('');

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		errorMessage = '';
		try {
			await onCreate(draft);
			draft = { ...draft, catalogVersionId: '' };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างชุดการเรียนไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_340px]">
	<section class="grid content-start gap-3 md:grid-cols-2">
		{#each offerings as offering (offering.id)}
			<article class="rounded-xl border bg-card p-5 shadow-sm">
				<div class="flex items-start justify-between gap-3">
					<div class="rounded-lg bg-primary/10 p-2 text-primary">
						{#if offering.kind === 'course'}<BookOpen class="size-5" />{:else}<Sparkles
								class="size-5"
							/>{/if}
					</div>
					<div class="flex gap-2">
						<Badge variant="outline">{offering.kind === 'course' ? 'รายวิชา' : 'กิจกรรม'}</Badge
						><Badge variant={offering.status === 'published' ? 'default' : 'secondary'}
							>{offering.status}</Badge
						>
					</div>
				</div>
				<h2 class="mt-4 font-semibold">{offering.nameSnapshot}</h2>
				<p class="mt-1 text-xs text-muted-foreground">
					{offering.codeSnapshot} · เป้าหมาย {offering.targets.length} กลุ่ม
				</p>
				<div class="mt-4 flex gap-2 border-t pt-3">
					<Button size="sm" variant="outline" onclick={() => onSelect(offering)}
						>จัดกลุ่มเรียน</Button
					>{#if canManage && offering.status === 'draft'}<Button
							size="sm"
							disabled={busy}
							onclick={() => onPublish(offering)}><CheckCircle2 class="size-4" /> เผยแพร่</Button
						>{/if}
				</div>
			</article>
		{:else}<div
				class="col-span-full rounded-xl border border-dashed p-10 text-center text-sm text-muted-foreground"
			>
				ยังไม่มีชุดการเรียนในภาคเรียนนี้
			</div>{/each}
	</section>

	{#if canManage}
		<form class="space-y-3 rounded-xl border bg-card p-5 shadow-sm" onsubmit={submit}>
			<h2 class="font-semibold">สร้างชุดการเรียนเอง</h2>
			<div class="space-y-1.5">
				<Label for="offering-kind">ชนิด</Label>
				<Select.Root type="single" bind:value={draft.kind}>
					<Select.Trigger id="offering-kind" class="w-full">
						{draft.kind === 'course' ? 'รายวิชา' : 'กิจกรรมพัฒนาผู้เรียน'}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="course">รายวิชา</Select.Item>
						<Select.Item value="activity">กิจกรรมพัฒนาผู้เรียน</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>
			<div class="space-y-1.5">
				<Label for="offering-version"
					>รหัสรุ่น{draft.kind === 'course' ? 'รายวิชา' : 'กิจกรรม'}</Label
				><Input id="offering-version" bind:value={draft.catalogVersionId} required />
			</div>
			<div class="space-y-1.5">
				<Label for="offering-owner">รหัสหน่วยงานเจ้าของ</Label><Input
					id="offering-owner"
					bind:value={draft.owningOrganizationUnitId}
					required
				/>
			</div>
			<div class="space-y-1.5">
				<Label for="offering-grade">รหัสระดับชั้น</Label><Input
					id="offering-grade"
					bind:value={draft.gradeLevelId}
					required
				/>
			</div>
			<div class="space-y-1.5">
				<Label for="offering-program">รหัสแผนการเรียน</Label><Input
					id="offering-program"
					bind:value={draft.studyProgramId}
					required
				/>
			</div>
			<Button class="w-full" type="submit" disabled={busy}
				><Plus class="size-4" /> สร้างฉบับร่าง</Button
			>
			<p class="text-xs leading-relaxed text-muted-foreground">
				นโยบายการประเมินใช้ค่าเริ่มต้นของโรงเรียน และจะบริหารในพื้นที่ประเมินผลโดยเฉพาะ
			</p>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</form>
	{/if}
</div>
