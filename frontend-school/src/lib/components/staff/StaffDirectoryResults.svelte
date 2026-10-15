<script lang="ts">
	import { Eye, Pencil, Trash2, UserRound } from '@lucide/svelte';
	import type { StaffListItem } from '#lib/api/staff.js';
	import { ACADEMIC_RANK_LABELS } from '#lib/forms/staff-personnel.js';
	import { staffStatusLabel } from '#lib/forms/staff-status.js';
	import { withStaffReturn } from '#lib/navigation/staff-management.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Table from '#lib/components/ui/table/index.js';

	let {
		staffList,
		view,
		directoryHref,
		canUpdate,
		canDelete,
		deleting,
		onDeactivate
	}: {
		staffList: StaffListItem[];
		view: 'list' | 'cards';
		directoryHref: string;
		canUpdate: boolean;
		canDelete: boolean;
		deleting: boolean;
		onDeactivate: (staff: StaffListItem) => void;
	} = $props();

	function displayName(staff: Pick<StaffListItem, 'title' | 'first_name' | 'last_name'>) {
		return `${staff.title}${staff.first_name} ${staff.last_name}`.trim();
	}
</script>

{#snippet portrait(staff: StaffListItem, large = false)}
	<div
		class={large
			? 'relative mx-auto flex size-24 shrink-0 items-center justify-center overflow-hidden rounded-full bg-primary/10 text-primary'
			: 'relative flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-full bg-primary/10 text-primary'}
	>
		<UserRound class={large ? 'size-10' : 'size-5'} aria-hidden="true" />
		{#if staff.profile_image_file_id}
			<PrivateFileImage
				fileId={staff.profile_image_file_id}
				resourceId={staff.id}
				alt={`รูป ${displayName(staff)}`}
				class="absolute inset-0 size-full object-cover"
			/>
		{/if}
	</div>
{/snippet}

{#snippet name(staff: StaffListItem)}
	<a
		href={withStaffReturn(`/staff/manage/${staff.id}`, directoryHref)}
		data-sveltekit-preload-data="tap"
		class="break-words font-medium text-foreground hover:text-primary hover:underline"
		>{displayName(staff)}</a
	>
	<p class="text-xs text-muted-foreground">{staff.username}</p>
{/snippet}

{#snippet position(staff: StaffListItem)}
	<p class="text-sm">{staff.job_position?.name ?? 'ยังไม่ระบุตำแหน่ง'}</p>
	<p class="text-xs text-muted-foreground">
		{staff.academic_rank ? ACADEMIC_RANK_LABELS[staff.academic_rank] : 'ยังไม่ระบุวิทยฐานะ'}
	</p>
{/snippet}

{#snippet roles(staff: StaffListItem)}
	<div class="flex flex-wrap gap-1">
		{#each staff.roles.slice(0, 2) as role (role)}
			<Badge variant="secondary">{role}</Badge>
		{:else}
			<span class="text-xs text-muted-foreground">ยังไม่มีบทบาท</span>
		{/each}
		{#if staff.roles.length > 2}
			<Badge variant="outline" title={staff.roles.slice(2).join(', ')}
				>+{staff.roles.length - 2}</Badge
			>
		{/if}
	</div>
{/snippet}

{#snippet organizations(staff: StaffListItem)}
	<p class="break-words text-sm">{staff.organization_units[0] ?? 'ยังไม่มีสังกัด'}</p>
	{#if staff.organization_units.length > 1}
		<p class="text-xs text-muted-foreground" title={staff.organization_units.slice(1).join(', ')}>
			อีก {staff.organization_units.length - 1} หน่วยงาน
		</p>
	{/if}
{/snippet}

{#snippet status(staff: StaffListItem)}
	<Badge variant={staff.status === 'active' ? 'default' : 'secondary'}>
		{staffStatusLabel(staff.status)}
	</Badge>
{/snippet}

{#snippet actions(staff: StaffListItem)}
	<div class="flex justify-end gap-1">
		<Button
			href={withStaffReturn(`/staff/manage/${staff.id}`, directoryHref)}
			data-sveltekit-preload-data="tap"
			variant="ghost"
			size="icon-sm"
			aria-label="ดูข้อมูล"
		>
			<Eye class="size-4" aria-hidden="true" />
		</Button>
		{#if canUpdate}
			<Button
				href={withStaffReturn(`/staff/manage/${staff.id}/edit`, directoryHref)}
				data-sveltekit-preload-data="tap"
				variant="ghost"
				size="icon-sm"
				aria-label="แก้ไข"
			>
				<Pencil class="size-4" aria-hidden="true" />
			</Button>
		{/if}
		{#if canDelete}
			<Button
				onclick={() => onDeactivate(staff)}
				disabled={deleting}
				variant="ghost"
				size="icon-sm"
				aria-label="ปิดการใช้งาน"
			>
				<Trash2 class="size-4" aria-hidden="true" />
			</Button>
		{/if}
	</div>
{/snippet}

{#if view === 'cards'}
	<ul class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4" aria-label="การ์ดบุคลากร">
		{#each staffList as staff (staff.id)}
			<li class="min-w-0">
				<Card.Root class="h-full gap-4">
					<Card.Content class="space-y-4">
						{@render portrait(staff, true)}
						<div class="space-y-1 text-center">{@render name(staff)}</div>
						<div class="flex justify-center">{@render status(staff)}</div>
						<div class="space-y-3 border-t pt-4">
							<div>{@render position(staff)}</div>
							<div>{@render organizations(staff)}</div>
							{@render roles(staff)}
						</div>
					</Card.Content>
					<Card.Footer class="mt-auto justify-end border-t pt-3"
						>{@render actions(staff)}</Card.Footer
					>
				</Card.Root>
			</li>
		{/each}
	</ul>
{:else}
	<Table.Root>
		<Table.Header>
			<Table.Row>
				<Table.Head>ชื่อ-นามสกุล</Table.Head>
				<Table.Head>ตำแหน่ง / วิทยฐานะ</Table.Head>
				<Table.Head>บทบาท</Table.Head>
				<Table.Head>สังกัด</Table.Head>
				<Table.Head>สถานะ</Table.Head>
				<Table.Head class="text-right">จัดการ</Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each staffList as staff (staff.id)}
				<Table.Row>
					<Table.Cell
						><div class="flex items-center gap-3">
							{@render portrait(staff)}
							<div>{@render name(staff)}</div>
						</div></Table.Cell
					>
					<Table.Cell>{@render position(staff)}</Table.Cell>
					<Table.Cell>{@render roles(staff)}</Table.Cell>
					<Table.Cell>{@render organizations(staff)}</Table.Cell>
					<Table.Cell>{@render status(staff)}</Table.Cell>
					<Table.Cell>{@render actions(staff)}</Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
{/if}
