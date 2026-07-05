<script lang="ts">
	import { onMount } from 'svelte';
	import { AlertTriangle, BellRing, CheckCircle2, RefreshCw, XCircle } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { notificationStore, type PushNotificationDeviceStatus } from '$lib/stores/notification';

	let status = $state<PushNotificationDeviceStatus | null>(null);
	let loadingStatus = $state(true);
	let enabling = $state(false);

	function statusLabel() {
		if (!status) return 'กำลังตรวจสอบ';
		if (!status.supported) return 'ไม่รองรับ';
		if (status.permission === 'denied') return 'ถูกปิด';
		if (status.hasSubscription) return 'พร้อมใช้งาน';
		if (status.permission === 'granted') return 'ต้องซิงก์';
		return 'ยังไม่ได้เปิด';
	}

	function statusClass() {
		if (!status) return 'border-muted-foreground/30 text-muted-foreground';
		if (!status.supported || status.permission === 'denied') {
			return 'border-destructive/30 bg-destructive/10 text-destructive';
		}
		if (status.hasSubscription) {
			return 'border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300';
		}
		return 'border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300';
	}

	function statusIcon() {
		if (!status) return RefreshCw;
		if (!status.supported || status.permission === 'denied') return XCircle;
		if (status.hasSubscription) return CheckCircle2;
		return AlertTriangle;
	}

	function statusDetail() {
		if (!status) return 'กำลังอ่านสถานะจากอุปกรณ์นี้';
		if (!status.supported) return 'เบราว์เซอร์นี้ยังไม่รองรับ Web Push';
		if (status.permission === 'denied') return 'การแจ้งเตือนถูกปิดในระบบหรือเบราว์เซอร์';
		if (status.hasSubscription) return 'อุปกรณ์นี้เชื่อมกับระบบแจ้งเตือนแล้ว';
		if (status.permission === 'granted')
			return 'อนุญาตแล้ว แต่ยังไม่มี subscription ที่ซิงก์กับระบบ';
		return 'ยังไม่ได้อนุญาตการแจ้งเตือนบนอุปกรณ์นี้';
	}

	async function refreshStatus(showToast = false) {
		loadingStatus = true;
		try {
			status = await notificationStore.getPushStatus();
			if (showToast) toast.success('อัปเดตสถานะการแจ้งเตือนแล้ว');
		} catch (error) {
			console.error('Failed to refresh push notification status', error);
			toast.error('ไม่สามารถอ่านสถานะการแจ้งเตือนได้');
		} finally {
			loadingStatus = false;
		}
	}

	async function enablePush() {
		enabling = true;
		try {
			const success = await notificationStore.enablePushFromUserAction(true);
			await refreshStatus();

			if (success) {
				toast.success('เปิดและซิงก์การแจ้งเตือนแล้ว');
			} else {
				toast.error('ยังไม่สามารถเปิดการแจ้งเตือนได้');
			}
		} finally {
			enabling = false;
		}
	}

	const StatusIcon = $derived(statusIcon());
	const canEnable = $derived(
		Boolean(status?.supported) && !(status?.isIOS && !status?.isStandalone)
	);

	onMount(() => {
		void refreshStatus();
	});
</script>

<Card>
	<CardHeader>
		<div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
			<div class="space-y-1.5">
				<CardTitle class="flex items-center gap-2">
					<BellRing class="h-5 w-5 text-primary" />
					การแจ้งเตือนบนอุปกรณ์นี้
				</CardTitle>
				<CardDescription>สถานะ Web Push ของเครื่องที่กำลังใช้งาน</CardDescription>
			</div>
			<Badge variant="outline" class={statusClass()}>
				<StatusIcon class={loadingStatus ? 'h-4 w-4 animate-spin' : 'h-4 w-4'} />
				{statusLabel()}
			</Badge>
		</div>
	</CardHeader>
	<CardContent class="space-y-4">
		<div class="rounded-md border bg-muted/40 p-4" aria-live="polite">
			<p class="text-sm font-medium">{statusDetail()}</p>
			{#if status}
				<div class="mt-3 grid gap-2 text-xs text-muted-foreground sm:grid-cols-3">
					<div>
						<span class="font-medium text-foreground">สิทธิ์:</span>
						{status.permission}
					</div>
					<div>
						<span class="font-medium text-foreground">Subscription:</span>
						{status.hasSubscription ? 'มี' : 'ไม่มี'}
					</div>
					<div>
						<span class="font-medium text-foreground">โหมดแอป:</span>
						{status.isStandalone ? 'ใช่' : 'ไม่ใช่'}
					</div>
				</div>
			{/if}
		</div>

		{#if status?.isIOS && !status.isStandalone}
			<div
				class="rounded-md border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-800 dark:text-amber-200"
			>
				iOS ต้องเปิดจากแอปที่ติดตั้งบนหน้าจอโฮมก่อน จึงจะเปิด Web Push ได้
			</div>
		{/if}

		<div class="flex flex-col gap-2 sm:flex-row">
			<LoadingButton
				onclick={enablePush}
				loading={enabling}
				loadingLabel="กำลังซิงก์..."
				disabled={!canEnable}
				class="gap-2"
			>
				<BellRing class="h-4 w-4" />
				เปิด/ซิงก์การแจ้งเตือน
			</LoadingButton>
			<Button
				variant="outline"
				onclick={() => refreshStatus(true)}
				disabled={loadingStatus || enabling}
				class="gap-2"
			>
				<RefreshCw class={loadingStatus ? 'h-4 w-4 animate-spin' : 'h-4 w-4'} />
				รีเฟรชสถานะ
			</Button>
		</div>
	</CardContent>
</Card>
