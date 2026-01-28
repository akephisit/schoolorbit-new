<script lang="ts">
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Label } from "$lib/components/ui/label";
    import { Textarea } from "$lib/components/ui/textarea";
    import * as Select from "$lib/components/ui/select";
    import { apiClient } from "$lib/api/client";
    import { lookupStaff, type StaffLookupItem } from "$lib/api/lookup";
    import { toast } from "svelte-sonner";
    import { onMount } from "svelte";
    import { notificationStore } from "$lib/stores/notification";

    let title = $state("ทดสอบแจ้งเตือน");
    let message = $state("ข้อความทดสอบแจ้งเตือน Real-time");
    let type = $state<"info" | "success" | "warning" | "error">("info");
    let link = $state("");
    let targetUserId = $state("self"); // "self", "all", or UUID
    let loading = $state(false);

    let staffList = $state<StaffLookupItem[]>([]);

    onMount(async () => {
        try {
            staffList = await lookupStaff({ activeOnly: true, limit: 100 });
        } catch (e) {
            console.error("Failed to load staff:", e);
        }
    });
  
    async function sendNotification() {
      loading = true;
      try {
        await apiClient.post("/api/notifications", {
          user_id: targetUserId === "self" || targetUserId === "all" ? undefined : targetUserId, // If sending to specific user
          // Note: Backend API might need adjustment to handle "all" specifically, 
          // but for now let's assume empty user_id = self (or handle as broadcast if implemented).
          // Actually, looking at backend, send logic:
          // If user_id is provided -> send to that user.
          // To send to self, provide own UUID? Or if endpoint handles it. 
          // Let's stick to simple "Send to Self" (which puts undefined? No, usually backend needs user_id).
          // Let's pass undefined if self (so backend uses current user from token? Or do we need to fetch profile?)
          
          // Debug: Assuming backend uses auth user if user_id missing?
          // No, backend handler usually requires user_id if it's a "send to user" API.
          // Let's keep it simple: We map "self" to empty (if backend supports) or just warn.
          // Actually, let's just send the payload.
          
          title,
          message,
          type: type,
          link: link || null
        });
        toast.success("ส่งแจ้งเตือนสำเร็จ");
      } catch (e: any) {
        toast.error("ส่งแจ้งเตือนล้มเหลว: " + (e.message || "Unknown error"));
      } finally {
        loading = false;
      }
    }

    async function enablePush() {
        loading = true;
        try {
            const success = await notificationStore.subscribeToPush();
            if (success) toast.success("เปิดแจ้งเตือนบนมือถือแล้ว (Web Push Enabled)");
            else toast.error("ไม่สามารถเปิดแจ้งเตือนได้ Check console.");
        } finally {
            loading = false;
        }
    }

    async function testLocal() {
        try {
            const reg = await navigator.serviceWorker.ready;
            
            if (!reg.showNotification) {
                toast.warning("ไม่พบฟังก์ชันแสดงแจ้งเตือน (iOS ต้อง 'Add to Home Screen' ก่อนใช้งาน)");
                return;
            }

            await reg.showNotification("🔔 Local Test", {
                body: "ถ้าเห็นข้อความนี้ แสดงว่ามือถือเครื่องนี้รับแจ้งเตือนได้ปกติครับ!",
                icon: "/icon-192.png",
                // @ts-ignore
                vibrate: [200, 100, 200],
            });
            toast.success("สั่งแสดงแจ้งเตือนแล้ว (เช็ค Notification Center)");
        } catch (e: any) {
            console.error(e);
            toast.error("ไม่สามารถแสดงแจ้งเตือนได้: " + e.message);
        }
    }
</script>

<div class="container mx-auto py-10 max-w-lg">
	<div class="space-y-6">
		<div class="flex flex-wrap gap-4 items-center justify-between mb-6">
			<h1 class="text-2xl font-bold">🔔 Notification Tester</h1>
			<div class="flex gap-2">
				<Button variant="outline" size="sm" onclick={testLocal} disabled={loading}>
					📱 Test Local
				</Button>
				<Button variant="outline" size="sm" onclick={enablePush} disabled={loading}>
					☁️ Enable/Update Web Push
				</Button>
			</div>
		</div>

		<div class="space-y-4 p-6 border rounded-lg bg-card shadow-sm">
			<div class="space-y-2">
				<Label>Title</Label>
				<Input bind:value={title} placeholder="หัวข้อแจ้งเตือน" />
			</div>

			<div class="space-y-2">
				<Label>Message</Label>
				<Textarea bind:value={message} placeholder="เนื้อหาแจ้งเตือน" />
			</div>

			<div class="space-y-2">
				<Label>Link (Optional)</Label>
				<Input bind:value={link} placeholder="/student/profile" />
			</div>

			<div class="space-y-2">
				<Label>Type</Label>
				<Select.Root type="single" bind:value={type}>
					<Select.Trigger>
						{type}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="info">Info</Select.Item>
						<Select.Item value="success">Success</Select.Item>
						<Select.Item value="warning">Warning</Select.Item>
						<Select.Item value="error">Error</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>

			<div class="space-y-2">
				<Label>Target User</Label>
				<Select.Root type="single" bind:value={targetUserId}>
					<Select.Trigger>
						{targetUserId === 'self'
							? 'Send to Myself'
							: staffList.find((s) => s.id === targetUserId)?.name || targetUserId}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="self">Send to Myself</Select.Item>
						{#each staffList as staff}
							<Select.Item value={staff.id}>{staff.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<Button class="w-full" onclick={sendNotification} disabled={loading}>
				{loading ? 'Sending...' : 'Send Notification'}
			</Button>
		</div>
	</div>
</div>
