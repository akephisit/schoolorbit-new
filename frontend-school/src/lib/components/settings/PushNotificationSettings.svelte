<script lang="ts">
	import { onMount } from 'svelte';
	import { authStore } from '#lib/stores/auth.js';
	import type { PushDevice } from '#lib/api/notifications.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { AlertTriangle, BellRing, CheckCircle2, RefreshCw, XCircle } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';

	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import {
		notificationStore,
		type PushNotificationDeviceStatus
	} from '#lib/stores/notification.js';

	let status = $state<PushNotificationDeviceStatus | null>(null);
	let loadingStatus = $state(true);
	let enabling = $state(false);
	let revoking = $state<string | null>(null);
	let requestVersion = 0;

	function statusLabel() {
		if (!status) return 'กำลังตรวจสอบ';
		if (!status.supported) return 'ไม่รองรับ';
		if (status.permission === 'denied') return 'ถูกปิด';
		if (status.error) return 'ตรวจสอบไม่ได้';
		if (!status.serverAvailable) return 'ระบบยังไม่พร้อม';
		if (status.synced) return 'เชื่อมต่อแล้ว';
		if (status.permission === 'granted') return 'ต้องซิงก์';
		return 'ยังไม่ได้เปิด';
	}

	function statusClass() {
		if (!status) return 'border-muted-foreground/30 text-muted-foreground';
		if (!status.supported || status.permission === 'denied') {
			return 'border-destructive/30 bg-destructive/10 text-destructive';
		}
		if (status.synced) {
			return 'border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300';
		}
		return 'border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300';
	}

	function statusIcon() {
		if (!status) return RefreshCw;
		if (!status.supported || status.permission === 'denied') return XCircle;
		if (status.synced) return CheckCircle2;
		return AlertTriangle;
	}

	function statusDetail() {
		if (!status) return 'กำลังอ่านสถานะจากอุปกรณ์นี้';
		if (!status.supported) return 'เบราว์เซอร์นี้ยังไม่รองรับ Web Push';
		if (status.permission === 'denied') return 'การแจ้งเตือนถูกปิดในระบบหรือเบราว์เซอร์';
		if (status.error) return status.error;
		if (!status.serverAvailable)
			return 'ระบบส่งแจ้งเตือนบนอุปกรณ์ยังไม่พร้อมใช้งาน กล่องแจ้งเตือนยังใช้งานได้';
		if (status.synced)
			return 'ยืนยันการเชื่อมต่ออุปกรณ์นี้กับระบบแล้ว การแจ้งเตือนทั้งหมดอยู่ในกล่องแจ้งเตือน';
		if (status.permission === 'granted')
			return 'อนุญาตแล้ว แต่ยังไม่มี subscription ที่ซิงก์กับระบบ';
		return 'ยังไม่ได้อนุญาตการแจ้งเตือนบนอุปกรณ์นี้';
	}

	async function refreshStatus() {
		const version = ++requestVersion;
		const epoch = authStore.sessionEpoch;
		loadingStatus = !status;
		try {
			const next = await notificationStore.getPushStatus();
			if (version === requestVersion && epoch === authStore.sessionEpoch) status = next;
		} catch (error) {
			console.error('Failed to refresh push notification status', error);
			toast.error('ไม่สามารถอ่านสถานะการแจ้งเตือนได้');
		} finally {
			if (version === requestVersion) loadingStatus = false;
		}
	}

	async function enablePush() {
		const epoch = authStore.sessionEpoch;
		enabling = true;
		try {
			const success = await notificationStore.enablePushFromUserAction();
			if (epoch !== authStore.sessionEpoch) return;
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
		Boolean(status?.supported && status.serverAvailable && !status.error) &&
			!(status?.isIOS && !status?.isStandalone)
	);

	async function revoke(device: PushDevice) {
		const epoch = authStore.sessionEpoch;
		revoking = device.id;
		try {
			await notificationStore.revokePushDevice(device);
			if (epoch !== authStore.sessionEpoch) return;
			await refreshStatus();
			toast.success('ยกเลิกการแจ้งเตือนบนอุปกรณ์แล้ว');
		} catch (error) {
			console.error('Failed to revoke notification device', error);
			toast.error('ไม่สามารถยกเลิกอุปกรณ์ได้');
		} finally {
			revoking = null;
		}
	}
	function formatDate(value: string) {
		return new Date(value).toLocaleString('th-TH', { dateStyle: 'medium', timeStyle: 'short' });
	}
	onMount(() => {
		let epoch = -1;
		const unsubscribe = authStore.subscribe(() => {
			if (epoch === authStore.sessionEpoch) return;
			epoch = authStore.sessionEpoch;
			requestVersion++;
			status = null;
			void refreshStatus();
		});
		return () => {
			requestVersion++;
			unsubscribe();
		};
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
				<CardDescription
					>กล่องแจ้งเตือนเก็บรายการจากระบบ อุปกรณ์ที่เชื่อมต่อช่วยเตือนเมื่อคุณไม่ได้เปิดแอป</CardDescription
				>
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
						<span class="font-medium text-foreground">อุปกรณ์นี้:</span>
						{status.synced ? 'เชื่อมต่อแล้ว' : 'ยังไม่ได้ยืนยัน'}
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
		</div>
		{#if status?.error}
			<Button variant="outline" onclick={refreshStatus} disabled={loadingStatus}
				>ลองตรวจสอบอีกครั้ง</Button
			>
		{/if}
		{#if status?.config}
			<div class="space-y-3 border-t pt-4">
				<h3 class="font-medium">
					อุปกรณ์ของคุณ ({status.devices.length}/{status.config.max_devices})
				</h3>
				<p class="text-xs text-muted-foreground">
					การเชื่อมต่อหมดอายุเมื่อไม่มีการยืนยันเป็นเวลา {status.config.device_expiry_days} วัน การยกเลิกเครื่องอื่นไม่ปิดการแจ้งเตือนบนเครื่องนี้
				</p>
				{#each status.devices as device (device.id)}
					<div
						class="flex flex-col gap-3 rounded-md border p-3 sm:flex-row sm:items-center sm:justify-between"
					>
						<div class="min-w-0 space-y-1">
							<p class="break-words text-sm font-medium">
								{device.label}
								{device.id === status.currentDeviceId ? '(เครื่องนี้)' : ''}
							</p>
							<p class="text-xs text-muted-foreground">
								ยืนยันล่าสุด {formatDate(device.last_confirmed_at)}
							</p>
							<p class="text-xs text-muted-foreground">หมดอายุ {formatDate(device.expires_at)}</p>
						</div>
						<LoadingButton
							variant="outline"
							size="sm"
							onclick={() => revoke(device)}
							loading={revoking === device.id}
							disabled={revoking !== null}
							loadingLabel="กำลังยกเลิก...">ยกเลิกอุปกรณ์</LoadingButton
						>
					</div>
				{:else}
					<p class="text-sm text-muted-foreground">ยังไม่มีอุปกรณ์ที่เชื่อมต่อ</p>
				{/each}
			</div>
		{/if}
	</CardContent>
</Card>
