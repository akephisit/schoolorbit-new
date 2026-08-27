<script lang="ts">
	import type {
		CreateLearningGroupRequest,
		DeliveryManagementOptions,
		LearningGroup
	} from '$lib/api/learning-delivery';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import { ArrowRight, Plus, UsersRound } from 'lucide-svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';

	let {
		groups,
		selectedGroupId = null,
		canManage = false,
		onSelect,
		onRequestManagementOptions,
		onCreate
	}: {
		groups: LearningGroup[];
		selectedGroupId?: string | null;
		canManage?: boolean;
		onSelect: (group: LearningGroup) => void;
		onRequestManagementOptions: () => Promise<DeliveryManagementOptions | null>;
		onCreate: (request: CreateLearningGroupRequest) => Promise<void>;
	} = $props();

	let createOpen = $state(false);
	let managementOptions = $state.raw<DeliveryManagementOptions | null>(null);
	let optionsLoading = $state(false);
	let saving = $state(false);
	let errorMessage = $state('');
	let preferredRoomId = $state('');
	let draft = $state({ code: '', name: '', description: '', capacity: null as number | null });

	async function showCreate() {
		createOpen = true;
		errorMessage = '';
		if (managementOptions || optionsLoading) return;
		optionsLoading = true;
		try {
			managementOptions = await onRequestManagementOptions();
		} catch (error) {
			errorMessage =
				error instanceof Error ? error.message : 'โหลดตัวเลือกสำหรับสร้างกลุ่มไม่สำเร็จ';
		} finally {
			optionsLoading = false;
		}
	}

	async function createGroup(event: SubmitEvent) {
		event.preventDefault();
		saving = true;
		errorMessage = '';
		try {
			await onCreate({
				code: draft.code.trim(),
				name: draft.name.trim(),
				description: draft.description.trim() || null,
				capacity: draft.capacity,
				preferredRoomIds: preferredRoomId ? [preferredRoomId] : []
			});
			draft = { code: '', name: '', description: '', capacity: null };
			preferredRoomId = '';
			createOpen = false;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างกลุ่มเรียนไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
	<header class="flex items-start justify-between gap-4 border-b bg-muted/25 p-4">
		<div>
			<h2 class="font-semibold">กลุ่มเรียน</h2>
			<p class="mt-1 text-sm text-muted-foreground">
				หนึ่งรายการเปิดสอนแบ่งเป็นหลายกลุ่มตามผู้เรียน ครู หรือเวลาเรียนจริงได้
			</p>
		</div>
		<div class="flex shrink-0 items-center gap-2">
			<Badge variant="secondary">{groups.length} กลุ่ม</Badge>{#if canManage}<Button
					size="sm"
					variant="outline"
					onclick={showCreate}><Plus class="size-4" /> เพิ่มกลุ่ม</Button
				>{/if}
		</div>
	</header>
	<div class="divide-y">
		{#each groups as group (group.id)}
			<Button
				type="button"
				variant="ghost"
				class="h-auto w-full justify-start rounded-none px-4 py-4 text-start font-normal hover:bg-muted/35"
				onclick={() => onSelect(group)}
			>
				<div class="flex w-full items-center gap-3">
					<div
						class={selectedGroupId === group.id
							? 'rounded-xl bg-primary p-2.5 text-primary-foreground'
							: 'rounded-xl bg-primary/10 p-2.5 text-primary'}
					>
						<UsersRound class="size-4" />
					</div>
					<div class="min-w-0 flex-1">
						<div class="flex flex-wrap items-center gap-2">
							<p class="font-medium">{group.name}</p>
							<span class="font-mono text-xs text-muted-foreground">{group.code}</span>
						</div>
						<p class="mt-1 text-xs text-muted-foreground">
							ครู {group.teacherAssignments.length} คน · ห้องต้นทาง {group.homeroomIds.length} ห้อง ·
							ห้องเรียนที่ต้องการ {group.preferredRoomIds.length} ห้อง
						</p>
					</div>
					<Badge variant={group.rosterStatus === 'published' ? 'default' : 'secondary'}
						>{group.rosterStatus === 'published' ? 'รายชื่อเผยแพร่แล้ว' : 'รายชื่อฉบับร่าง'}</Badge
					><ArrowRight class="size-4 shrink-0 text-muted-foreground" />
				</div>
			</Button>
		{:else}
			<div class="p-8 text-center text-sm text-muted-foreground">
				ยังไม่มีกลุ่มเรียน เริ่มจากเพิ่มกลุ่มตามห้องเรียนหรือชุดผู้เรียนจริง
			</div>
		{/each}
	</div>
</section>

<Dialog.Root bind:open={createOpen}>
	<Dialog.Content class="sm:max-w-xl">
		<Dialog.Header
			><Dialog.Title>เพิ่มกลุ่มเรียน</Dialog.Title><Dialog.Description
				>ตั้งชื่อให้ครูค้นหาได้ง่าย จากนั้นค่อยกำหนดครู ห้องต้นทาง และรายชื่อ</Dialog.Description
			></Dialog.Header
		>
		{#if optionsLoading}<div class="space-y-3 py-3" aria-label="กำลังโหลดตัวเลือก">
				<div class="h-10 animate-pulse rounded-md bg-muted"></div>
				<div class="h-24 animate-pulse rounded-md bg-muted"></div>
			</div>{:else if managementOptions}
			<form class="space-y-4" onsubmit={createGroup}>
				<div class="grid gap-4 sm:grid-cols-2">
					<div class="space-y-2">
						<Label for="delivery-group-code">รหัสกลุ่ม</Label><Input
							id="delivery-group-code"
							bind:value={draft.code}
							placeholder="เช่น ม.1/1-A"
							required
						/>
					</div>
					<div class="space-y-2">
						<Label for="delivery-group-capacity">ความจุ (ถ้ามี)</Label><Input
							id="delivery-group-capacity"
							type="number"
							min="1"
							bind:value={draft.capacity}
						/>
					</div>
				</div>
				<div class="space-y-2">
					<Label for="delivery-group-name">ชื่อกลุ่ม</Label><Input
						id="delivery-group-name"
						bind:value={draft.name}
						placeholder="เช่น ม.1/1 ห้องเรียนปกติ"
						required
					/>
				</div>
				<div class="space-y-2">
					<Label for="delivery-group-description">คำอธิบาย (ถ้ามี)</Label><Textarea
						id="delivery-group-description"
						bind:value={draft.description}
						rows={2}
					/>
				</div>
				<div class="space-y-2">
					<Label>ห้องเรียนที่ต้องการ (ถ้ามี)</Label><DeliveryOptionCombobox
						bind:value={preferredRoomId}
						options={managementOptions.rooms.map((room) => ({
							id: room.id,
							label: room.name_th,
							description: [room.code, room.building_name].filter(Boolean).join(' · ')
						}))}
						placeholder="ยังไม่กำหนดห้อง"
						searchPlaceholder="ค้นหาชื่อหรือรหัสห้อง..."
					/>
				</div>
				{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
				<Dialog.Footer
					><Button type="button" variant="outline" onclick={() => (createOpen = false)}
						>ยกเลิก</Button
					><LoadingButton
						type="submit"
						loading={saving}
						loadingLabel="กำลังสร้าง"
						disabled={!draft.code.trim() || !draft.name.trim()}>สร้างกลุ่มเรียน</LoadingButton
					></Dialog.Footer
				>
			</form>
		{:else}<div class="space-y-3 py-4">
				<p role="alert" class="text-sm text-destructive">
					{errorMessage || 'ไม่สามารถโหลดตัวเลือกได้'}
				</p>
				<Button type="button" variant="outline" onclick={showCreate}>ลองอีกครั้ง</Button>
			</div>{/if}
	</Dialog.Content>
</Dialog.Root>
