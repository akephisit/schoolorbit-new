<script lang="ts">
	import type { LearningGroup, LearningOffering } from '$lib/api/learning-delivery';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Plus, UsersRound } from 'lucide-svelte';

	let {
		offering,
		groups,
		homeroomOptions,
		staffOptions,
		canManage = false,
		onCreate,
		onConfigure,
		onPreviewRoster
	}: {
		offering: LearningOffering;
		groups: LearningGroup[];
		homeroomOptions: Array<{ id: string; name: string }>;
		staffOptions: Array<{ id: string; name: string }>;
		canManage?: boolean;
		onCreate: (draft: {
			code: string;
			name: string;
			description: string;
			capacity: number | null;
			preferredRoomIds: string[];
		}) => Promise<void>;
		onConfigure: (
			group: LearningGroup,
			draft: {
				homeroomIds: string[];
				preferredRoomIds: string[];
				teacherId: string;
				teacherRole: 'primary' | 'secondary' | 'assistant';
			}
		) => Promise<void>;
		onPreviewRoster: (group: LearningGroup) => void;
	} = $props();

	let draft = $state({
		code: '',
		name: '',
		description: '',
		capacity: null as number | null,
		preferredRoomIds: [] as string[]
	});
	let roomIdsText = $state('');
	let busy = $state(false);
	let errorMessage = $state('');
	let configureDraft = $state({
		groupId: '',
		homeroomIds: [] as string[],
		preferredRoomIds: [] as string[],
		teacherId: '',
		teacherRole: 'primary' as 'primary' | 'secondary' | 'assistant'
	});
	let homeroomIdsText = $state('');
	let configureRoomIdsText = $state('');

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		errorMessage = '';
		try {
			await onCreate({
				...draft,
				preferredRoomIds: roomIdsText
					.split(',')
					.map((id) => id.trim())
					.filter(Boolean)
			});
			draft = { ...draft, code: '', name: '', description: '' };
			roomIdsText = '';
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างกลุ่มเรียนไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function configure(event: SubmitEvent) {
		event.preventDefault();
		const group = groups.find((item) => item.id === configureDraft.groupId);
		if (!group) return;
		busy = true;
		errorMessage = '';
		try {
			await onConfigure(group, {
				...configureDraft,
				homeroomIds: homeroomIdsText
					.split(',')
					.map((id) => id.trim())
					.filter(Boolean),
				preferredRoomIds: configureRoomIdsText
					.split(',')
					.map((id) => id.trim())
					.filter(Boolean)
			});
			configureDraft = { ...configureDraft, teacherId: '' };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ตั้งค่ากลุ่มเรียนไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<section class="rounded-xl border bg-card">
	<header class="border-b px-5 py-4">
		<h2 class="font-semibold">กลุ่มเรียน · {offering.nameSnapshot}</h2>
		<p class="text-xs text-muted-foreground">{offering.codeSnapshot}</p>
	</header>
	<div class="grid gap-5 p-5 lg:grid-cols-[minmax(0,1fr)_300px]">
		<div class="space-y-3">
			{#each groups as group (group.id)}<article
					class="flex flex-wrap items-center justify-between gap-3 rounded-lg border p-4"
				>
					<div class="flex items-center gap-3">
						<div class="rounded-lg bg-primary/10 p-2 text-primary">
							<UsersRound class="size-4" />
						</div>
						<div>
							<h3 class="font-medium">{group.name}</h3>
							<p class="text-xs text-muted-foreground">
								{group.code} · ห้องต้นทาง {group.homeroomIds.length} · ครู {group.teacherAssignments
									.length}
							</p>
						</div>
					</div>
					<div class="flex items-center gap-2">
						<Badge variant="outline">{group.rosterStatus}</Badge><Button
							size="sm"
							variant="outline"
							onclick={() => onPreviewRoster(group)}>ตรวจรายชื่อ</Button
						>
					</div>
				</article>{:else}<p
					class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
				>
					ยังไม่มีกลุ่มเรียน
				</p>{/each}
		</div>
		{#if canManage}<div class="space-y-4">
				<form class="space-y-3 rounded-lg border bg-muted/20 p-4" onsubmit={submit}>
					<h3 class="font-medium">เพิ่มกลุ่มเรียน</h3>
					<div class="grid grid-cols-2 gap-3">
						<div class="space-y-1.5">
							<Label for="group-code">รหัส</Label><Input
								id="group-code"
								bind:value={draft.code}
								required
							/>
						</div>
						<div class="space-y-1.5">
							<Label for="group-capacity">ความจุ</Label><Input
								id="group-capacity"
								type="number"
								min="1"
								bind:value={draft.capacity}
							/>
						</div>
					</div>
					<div class="space-y-1.5">
						<Label for="group-name">ชื่อกลุ่ม</Label><Input
							id="group-name"
							bind:value={draft.name}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for="group-rooms">รหัสห้องที่ต้องการ (คั่นด้วยจุลภาค)</Label><Input
							id="group-rooms"
							bind:value={roomIdsText}
						/>
					</div>
					<Button class="w-full" type="submit" disabled={busy}
						><Plus class="size-4" /> เพิ่มกลุ่ม</Button
					>
				</form>
				<form class="space-y-3 rounded-lg border bg-muted/20 p-4" onsubmit={configure}>
					<h3 class="font-medium">ห้องต้นทาง ครู และห้องเรียน</h3>
					<label class="space-y-1.5 text-sm"
						><span class="font-medium">กลุ่มเรียน</span><select
							class="h-10 w-full rounded-md border bg-background px-3"
							bind:value={configureDraft.groupId}
							required
							><option value="">เลือกกลุ่ม</option>{#each groups as group (group.id)}<option
									value={group.id}>{group.name}</option
								>{/each}</select
						></label
					>
					<div class="space-y-1.5">
						<Label for="configure-homerooms">รหัสห้องต้นทาง (คั่นด้วยจุลภาค)</Label><Input
							id="configure-homerooms"
							list="homeroom-options"
							bind:value={homeroomIdsText}
							required
						/><datalist id="homeroom-options"
							>{#each homeroomOptions as option (option.id)}<option value={option.id}
									>{option.name}</option
								>{/each}</datalist
						>
					</div>
					<div class="space-y-1.5">
						<Label for="configure-rooms">รหัสห้องเรียนที่ต้องการ</Label><Input
							id="configure-rooms"
							bind:value={configureRoomIdsText}
						/>
					</div>
					<label class="space-y-1.5 text-sm"
						><span class="font-medium">เพิ่มครูผู้สอน</span><select
							class="h-10 w-full rounded-md border bg-background px-3"
							bind:value={configureDraft.teacherId}
							required
							><option value="">เลือกครู</option>{#each staffOptions as option (option.id)}<option
									value={option.id}>{option.name}</option
								>{/each}</select
						></label
					><label class="space-y-1.5 text-sm"
						><span class="font-medium">บทบาท</span><select
							class="h-10 w-full rounded-md border bg-background px-3"
							bind:value={configureDraft.teacherRole}
							><option value="primary">ผู้สอนหลัก</option><option value="secondary"
								>ผู้สอนร่วม</option
							><option value="assistant">ผู้ช่วย</option></select
						></label
					><Button class="w-full" type="submit" variant="outline" disabled={busy}
						>บันทึกการจัดกลุ่ม</Button
					>
				</form>
				{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
			</div>{/if}
	</div>
</section>
