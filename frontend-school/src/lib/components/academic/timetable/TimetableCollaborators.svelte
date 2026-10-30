<script lang="ts">
	import type { TimetableCollaborator } from '#lib/academic/timetable/collaboration-presentation.js';
	import { collaboratorInitials } from '#lib/academic/timetable/collaboration-presentation.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import { Avatar } from '#lib/components/ui/avatar/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import * as Tooltip from '#lib/components/ui/tooltip/index.js';
	import { ChevronDown } from '@lucide/svelte';

	let {
		users,
		disabled = false,
		onJump
	}: {
		users: TimetableCollaborator[];
		disabled?: boolean;
		onJump: (connectionId: string) => void;
	} = $props();
	const visible = $derived(users.slice(0, 3));
	const needsMenu = $derived(users.length > 3 || users.some((user) => user.tabs.length > 1));
</script>

<div
	class="inline-flex h-8 shrink-0 items-center gap-0.5"
	aria-label="ผู้ร่วมจัดตาราง"
	data-timetable-roster
>
	<Tooltip.Provider>
		{#each visible as user (user.userId)}
			{@const firstTab = user.tabs.find((tab) => tab.canJump) ?? user.tabs[0]}
			{#if firstTab}
				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<Button
								{...props}
								variant="ghost"
								size="icon-sm"
								class="rounded-full"
								{disabled}
								aria-label={`ไปยัง ${user.name} · ${firstTab.contextLabel}`}
								data-timetable-collaborator={user.userId}
								onclick={() => onJump(firstTab.connectionId)}
							>
								<span
									class="relative block size-7 overflow-hidden rounded-full border-2"
									style:border-color={user.color}
								>
									<Avatar
										size="xs"
										class="size-full"
										initials={collaboratorInitials(user.name)}
										alt={user.name}
									/>
									{#if user.profileImageFileId}<PrivateFileImage
											fileId={user.profileImageFileId}
											resourceId={user.userId}
											alt={`รูป ${user.name}`}
											class="absolute inset-0 size-full object-cover"
										/>{/if}
								</span>
							</Button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content class="max-w-72">
						<p class="font-medium">{user.name}</p>
						{#each user.tabs as tab (tab.connectionId)}<p class="mt-1">
								{tab.contextLabel} · {tab.activityLabel}
							</p>{/each}
					</Tooltip.Content>
				</Tooltip.Root>
			{/if}
		{/each}
	</Tooltip.Provider>
	{#if needsMenu}
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				{#snippet child({ props })}
					<Button
						{...props}
						variant="ghost"
						size="icon-sm"
						class="shrink-0 rounded-full text-xs"
						{disabled}
						aria-label="เลือกผู้ร่วมจัดตารางและแท็บ"
					>
						{#if users.length > 3}+{users.length - 3}{:else}<ChevronDown class="size-3" />{/if}
					</Button>
				{/snippet}
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="start" class="max-h-80 w-80 overflow-y-auto">
				{#each users as user (user.userId)}
					<DropdownMenu.Label
						>{user.name}{user.tabs.length > 1
							? ` · ${user.tabs.length} แท็บ`
							: ''}</DropdownMenu.Label
					>
					{#each user.tabs as tab, index (tab.connectionId)}
						<DropdownMenu.Item disabled={!tab.canJump} onclick={() => onJump(tab.connectionId)}>
							<span class="min-w-0 whitespace-normal"
								><span class="block"
									>{user.tabs.length > 1 ? `แท็บ ${index + 1} · ` : ''}{tab.contextLabel}</span
								><span class="block text-xs text-muted-foreground">{tab.activityLabel}</span></span
							>
						</DropdownMenu.Item>
					{/each}
				{/each}
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	{/if}
</div>
