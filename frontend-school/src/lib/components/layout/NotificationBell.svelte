<script lang="ts">
  import { onMount } from 'svelte';
  import { notificationStore } from '$lib/stores/notification';
  import { Bell, CheckCheck } from 'lucide-svelte';
  import { fly, fade } from 'svelte/transition';
  import * as Popover from '$lib/components/ui/popover';
  import { Button, buttonVariants } from '$lib/components/ui/button';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { Badge } from '$lib/components/ui/badge';

  onMount(() => {
    notificationStore.fetchNotifications();
    notificationStore.initSSE();

    return () => {
        notificationStore.closeSSE();
    };
  });

  function formatDate(dateStr: string) {
    const d = new Date(dateStr);
    const now = new Date();
    const diff = (now.getTime() - d.getTime()) / 1000;
    
    if (diff < 60) return 'เมื่อสักครู่';
    if (diff < 3600) return `${Math.floor(diff / 60)} นาทีที่แล้ว`;
    if (diff < 86400) return `${Math.floor(diff / 3600)} ชั่วโมงที่แล้ว`;
    return d.toLocaleDateString('th-TH', { day: 'numeric', month: 'short' });
  }

  function getTypeColor(type: string) {
      switch(type) {
          case 'success': return 'text-green-600 bg-green-50';
          case 'warning': return 'text-amber-600 bg-amber-50';
          case 'error': return 'text-red-600 bg-red-50';
          default: return 'text-blue-600 bg-blue-50';
      }
  }
</script>

<Popover.Root>
	<Popover.Trigger class={buttonVariants({ variant: 'ghost', size: 'icon' }) + ' relative'}>
		<Bell class="h-5 w-5 text-gray-600" />
		{#if $notificationStore.unreadCount > 0}
			<span
				class="absolute top-2 right-2 h-2.5 w-2.5 rounded-full bg-red-500 ring-2 ring-white animate-pulse"
				transition:fade
			></span>
		{/if}
	</Popover.Trigger>

	<Popover.Content class="w-80 p-0" align="end">
		<div class="flex items-center justify-between px-4 py-3 border-b">
			<h4 class="font-semibold">การแจ้งเตือน</h4>
			{#if $notificationStore.unreadCount > 0}
				<Button
					variant="ghost"
					size="sm"
					class="h-auto px-2 py-1 text-xs text-blue-600 hover:text-blue-700"
					onclick={() => notificationStore.markAllAsRead()}
				>
					<CheckCheck class="w-3 h-3 mr-1" />
					อ่านทั้งหมด
				</Button>
			{/if}
		</div>

		<ScrollArea class="h-[350px]">
			{#if $notificationStore.notifications.length === 0}
				<div class="flex flex-col items-center justify-center h-40 text-gray-500">
					<Bell class="w-8 h-8 mb-2 opacity-20" />
					<span class="text-sm">ไม่มีการแจ้งเตือน</span>
				</div>
			{:else}
				<div class="flex flex-col divide-y">
					{#each $notificationStore.notifications as notif (notif.id)}
						<button
							class="flex flex-col items-start w-full px-4 py-3 text-left transition-colors hover:bg-gray-50 {notif.read_at
								? 'opacity-60'
								: 'bg-blue-50/30'}"
							onclick={() => {
								if (!notif.read_at) notificationStore.markAsRead(notif.id);
								if (notif.link) window.location.href = notif.link;
							}}
						>
							<div class="flex items-start justify-between w-full mb-1">
								<span
									class="text-sm font-medium {notif.read_at ? 'text-gray-700' : 'text-gray-900'}"
								>
									{notif.title}
								</span>
								<span class="text-[10px] text-gray-400 whitespace-nowrap ml-2">
									{formatDate(notif.created_at)}
								</span>
							</div>
							<p class="text-xs text-gray-500 line-clamp-2">{notif.message}</p>
						</button>
					{/each}
				</div>
			{/if}
		</ScrollArea>
	</Popover.Content>
</Popover.Root>
