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
  
    let title = $state("ทดสอบแจ้งเตือน");
    let message = $state("ข้อความทดสอบแจ้งเตือน Real-time");
    let type = $state("info");
    let link = $state("");
    let targetUserId = $state("");
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
          user_id: targetUserId || undefined, // Empty = self
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
</script>

<div class="container mx-auto py-10 max-w-lg">
	<h1 class="text-2xl font-bold mb-6">🔔 Notification Tester</h1>

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
			<Label>Type</Label>
			<select class="w-full border rounded p-2 bg-background text-sm" bind:value={type}>
				<option value="info">ℹ️ Info (Blue)</option>
				<option value="success">✅ Success (Green)</option>
				<option value="warning">⚠️ Warning (Yellow)</option>
				<option value="error">❌ Error (Red)</option>
			</select>
		</div>

		<div class="space-y-2">
			<Label>Link (Optional)</Label>
			<Input bind:value={link} placeholder="/some/path" />
		</div>

		<div class="space-y-2">
			<Label>Target User (Recipient)</Label>
			<select class="w-full border rounded p-2 bg-background text-sm" bind:value={targetUserId}>
				<option value="">👤 ส่งหาตัวเอง (Me)</option>
				<option disabled>--- เลือกบุคลากร ---</option>
				{#each staffList as staff}
					<option value={staff.id}>{staff.name} {staff.title ? `(${staff.title})` : ''}</option>
				{/each}
			</select>
			<p class="text-xs text-muted-foreground">เลือกผู้รับ (ดึงจาก Lookup Staff)</p>
		</div>

		<Button onclick={sendNotification} disabled={loading} class="w-full">
			{loading ? 'Sending...' : 'Send Notification 🚀'}
		</Button>
	</div>
</div>
