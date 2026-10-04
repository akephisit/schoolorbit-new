<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onDestroy, untrack } from 'svelte';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { authAPI, type SessionDto } from '#lib/api/auth.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { keepCurrentSession, passwordValidation, removeRevokedSession } from './session-state';
	import { Clock3, KeyRound, Laptop, LogOut, ShieldCheck, Trash2 } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let {
		source,
		requestKey
	}: {
		source: Promise<RouteLoadResult<{ ownerKey: string; sessions: SessionDto[] }>>;
		requestKey: string;
	} = $props();
	const identityKey = $derived.by(() => {
		void $authStore;
		void $can;
		return appIdentityKey();
	});
	const ownerKey = $derived(`${identityKey}|${requestKey}`);
	const allowed = $derived($authStore.isAuthenticated);
	let sessions = $state.raw<SessionDto[]>([]);
	let sessionsLoaded = $state(false),
		isLoading = $state(true),
		loadError = $state('');
	let revokingSessionId = $state<string | null>(null);
	let isLoggingOutAll = $state(false),
		isChangingPassword = $state(false);
	let currentPassword = $state(''),
		newPassword = $state(''),
		confirmPassword = $state('');
	let selectedCurrentSession = $state<SessionDto | null>(null);
	let currentSessionDialogOpen = $state(false),
		logoutAllDialogOpen = $state(false);
	const actionBusy = $derived(revokingSessionId !== null || isLoggingOutAll || isChangingPassword);
	const sessionRequest = new LatestRequest();
	let consumedSource: typeof source | null = null;
	let owner = '',
		disposed = false;
	$effect.pre(() => {
		const key = ownerKey,
			read = source,
			canRead = allowed;
		untrack(() => {
			if (owner !== key || !canRead) {
				owner = key;
				sessionRequest.abort();
				sessions = [];
				sessionsLoaded = false;
				isLoading = canRead;
				loadError = '';
				currentPassword = '';
				newPassword = '';
				confirmPassword = '';
				selectedCurrentSession = null;
				currentSessionDialogOpen = false;
				logoutAllDialogOpen = false;
				revokingSessionId = null;
				isLoggingOutAll = false;
				isChangingPassword = false;
			}
			if (!canRead || consumedSource === read) return;
			consumedSource = read;
			const request = sessionRequest.begin();
			isLoading = true;
			void read.then((result) => applySessions(result, request.revision, key));
		});
	});
	onDestroy(() => {
		disposed = true;
		sessionRequest.abort();
		currentPassword = '';
		newPassword = '';
		confirmPassword = '';
	});
	function current(key: string) {
		return !disposed && allowed && ownerKey === key;
	}
	function applySessions(result: Awaited<typeof source>, revision: number, key: string) {
		if (!current(key) || !sessionRequest.isCurrent(revision)) return;
		isLoading = false;
		if (!result.ok) {
			loadError = result.error;
			return;
		}
		if (result.data.ownerKey !== key) return;
		sessions = result.data.sessions;
		sessionsLoaded = true;
		loadError = '';
	}
	function suspendSessionRead() {
		sessionRequest.abort();
		isLoading = !sessionsLoaded;
		loadError = '';
	}

	const dateTimeFormatter = new Intl.DateTimeFormat('th-TH', {
		dateStyle: 'medium',
		timeStyle: 'short'
	});

	function formatDateTime(value: string): string {
		const date = new Date(value);
		return Number.isNaN(date.getTime()) ? '-' : dateTimeFormatter.format(date);
	}

	function errorMessage(error: unknown, fallback: string): string {
		return error instanceof Error ? error.message : fallback;
	}

	async function loadSessions() {
		if (!allowed || disposed) return;
		const key = ownerKey,
			request = sessionRequest.begin();
		isLoading = true;
		loadError = '';
		const result = await captureRouteLoad(
			authAPI
				.listSessions({ signal: request.signal })
				.then((sessions) => ({ ownerKey: key, sessions })),
			'ไม่สามารถโหลดรายการอุปกรณ์ได้'
		);
		applySessions(result, request.revision, key);
	}

	async function revokeOtherSession(session: SessionDto) {
		if (
			!allowed ||
			disposed ||
			actionBusy ||
			session.isCurrent ||
			!sessions.some((row) => row.id === session.id)
		)
			return;
		const key = ownerKey;
		suspendSessionRead();
		revokingSessionId = session.id;
		try {
			await authAPI.revokeSession(session.id);
			if (!current(key)) return;
			sessions = removeRevokedSession(sessions, session.id);
			toast.success('นำอุปกรณ์ออกจากบัญชีแล้ว');
		} catch (error) {
			if (current(key)) toast.error(errorMessage(error, 'ไม่สามารถนำอุปกรณ์ออกได้'));
		} finally {
			if (current(key)) revokingSessionId = null;
		}
	}
	function requestCurrentSessionLogout(session: SessionDto) {
		if (!allowed || disposed || actionBusy || !session.isCurrent) return;
		selectedCurrentSession = session;
		currentSessionDialogOpen = true;
	}
	async function revokeCurrentSession() {
		const session = selectedCurrentSession;
		if (!session || !allowed || disposed || actionBusy) return;
		const key = ownerKey,
			epoch = authStore.sessionEpoch;
		suspendSessionRead();
		revokingSessionId = session.id;
		try {
			await authAPI.revokeSession(session.id, { current: true });
		} catch (error) {
			if (current(key)) toast.error(errorMessage(error, 'ไม่สามารถออกจากระบบอุปกรณ์นี้ได้'));
			return;
		} finally {
			if (current(key)) revokingSessionId = null;
		}
		if ($authStore.isAuthenticated || authStore.sessionEpoch !== epoch + 1) return;
		sessionStorage.removeItem('redirectAfterLogin');
		await goto(resolve('login'), { refreshAll: true });
	}
	async function logoutAllSessions() {
		if (!allowed || disposed || actionBusy) return;
		const key = ownerKey,
			epoch = authStore.sessionEpoch;
		suspendSessionRead();
		isLoggingOutAll = true;
		try {
			await authAPI.logoutAll();
		} catch (error) {
			if (current(key)) toast.error(errorMessage(error, 'ไม่สามารถออกจากระบบทุกอุปกรณ์ได้'));
			return;
		} finally {
			if (current(key)) isLoggingOutAll = false;
		}
		if ($authStore.isAuthenticated || authStore.sessionEpoch !== epoch + 1) return;
		sessionStorage.removeItem('redirectAfterLogin');
		await goto(resolve('login'), { refreshAll: true });
	}
	async function changePassword(event: SubmitEvent) {
		event.preventDefault();
		if (!allowed || disposed || actionBusy) return;
		const validationError = passwordValidation(currentPassword, newPassword, confirmPassword);
		if (validationError) {
			toast.error(validationError);
			return;
		}
		const key = ownerKey;
		suspendSessionRead();
		isChangingPassword = true;
		try {
			await authAPI.changePassword({ currentPassword, newPassword });
			if (!current(key)) return;
			sessions = keepCurrentSession(sessions);
			currentPassword = '';
			newPassword = '';
			confirmPassword = '';
			toast.success('เปลี่ยนรหัสผ่านสำเร็จ อุปกรณ์อื่นถูกนำออกจากบัญชีแล้ว');
			if (!sessionsLoaded) await loadSessions();
		} catch (error) {
			if (!current(key)) return;
			toast.error(errorMessage(error, 'ไม่สามารถเปลี่ยนรหัสผ่านได้'));
			if (!sessionsLoaded) await loadSessions();
		} finally {
			if (current(key)) isChangingPassword = false;
		}
	}
</script>

<div class="grid gap-6 xl:grid-cols-[minmax(0,1.35fr)_minmax(20rem,0.65fr)]">
	<Card>
		<CardHeader class="gap-3 sm:flex-row sm:items-start sm:justify-between">
			<div class="space-y-1.5">
				<CardTitle class="flex items-center gap-2">
					<Laptop class="h-5 w-5" />
					อุปกรณ์ที่เข้าสู่ระบบ
				</CardTitle>
				<CardDescription>
					ตรวจสอบและนำอุปกรณ์ที่ไม่รู้จักออกจากบัญชี อุปกรณ์นี้จะแสดงด้วยป้ายกำกับ
				</CardDescription>
			</div>
			<Button
				data-testid="logout-all-sessions"
				variant="destructive"
				size="sm"
				disabled={isLoading || sessions.length === 0 || actionBusy}
				onclick={() => (logoutAllDialogOpen = true)}
			>
				<LogOut class="h-4 w-4" />
				ออกจากระบบทุกอุปกรณ์
			</Button>
		</CardHeader>
		<CardContent aria-busy={isLoading}>
			<Button variant="outline" disabled={isLoading || actionBusy} onclick={loadSessions}
				>โหลดข้อมูลใหม่</Button
			>
			{#if isLoading && sessionsLoaded}<p
					role="status"
					aria-label="กำลังอัปเดตรายการอุปกรณ์"
					class="text-muted-foreground text-sm"
				>
					กำลังอัปเดตรายการอุปกรณ์…
				</p>{/if}
			{#if loadError}
				<PageState
					variant="error"
					title="โหลดรายการอุปกรณ์ไม่สำเร็จ"
					description={loadError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadSessions}
				/>
			{/if}
			{#if isLoading && !sessionsLoaded}
				<div role="status" aria-label="กำลังโหลดรายการอุปกรณ์">
					<PageSkeleton variant="cards" rows={2} />
				</div>
			{:else if sessionsLoaded && sessions.length === 0}
				<PageState
					title="ยังไม่มีอุปกรณ์ที่เข้าสู่ระบบ"
					description="เมื่อมีการเข้าสู่ระบบ อุปกรณ์จะแสดงที่นี่"
				/>
			{:else if sessionsLoaded}
				<div data-testid="session-list" class="divide-y rounded-lg border">
					{#each sessions as session (session.id)}
						<div
							data-testid={`session-row-${session.id}`}
							data-current={session.isCurrent}
							class="flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:justify-between"
						>
							<div class="min-w-0 space-y-2">
								<div class="flex flex-wrap items-center gap-2">
									<p class="truncate font-medium">{session.deviceLabel}</p>
									{#if session.isCurrent}
										<Badge variant="secondary">
											<ShieldCheck class="h-3 w-3" />
											อุปกรณ์นี้
										</Badge>
									{/if}
									<Badge variant="outline">
										{session.rememberMe ? 'จดจำการเข้าสู่ระบบ' : 'เซสชันปกติ'}
									</Badge>
								</div>
								<div class="text-muted-foreground grid gap-1 text-xs sm:grid-cols-2">
									<span class="flex items-center gap-1.5">
										<Clock3 class="h-3.5 w-3.5" />
										ใช้งานล่าสุด {formatDateTime(session.lastSeenAt)}
									</span>
									<span>เข้าสู่ระบบเมื่อ {formatDateTime(session.createdAt)}</span>
									<span>หมดอายุเมื่อ {formatDateTime(session.idleExpiresAt)}</span>
									<span>หมดอายุสูงสุด {formatDateTime(session.absoluteExpiresAt)}</span>
								</div>
							</div>

							{#if session.isCurrent}
								<Button
									variant="outline"
									size="sm"
									disabled={actionBusy}
									onclick={() => requestCurrentSessionLogout(session)}
								>
									<LogOut class="h-4 w-4" />
									ออกจากระบบอุปกรณ์นี้
								</Button>
							{:else}
								<LoadingButton
									variant="outline"
									size="sm"
									disabled={actionBusy}
									loading={revokingSessionId === session.id}
									loadingLabel="กำลังนำออก..."
									onclick={() => revokeOtherSession(session)}
								>
									<Trash2 class="h-4 w-4" />
									นำอุปกรณ์ออก
								</LoadingButton>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</CardContent>
	</Card>

	<Card class="h-fit">
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<KeyRound class="h-5 w-5" />
				เปลี่ยนรหัสผ่าน
			</CardTitle>
			<CardDescription>
				หลังเปลี่ยนรหัสผ่าน ระบบจะนำอุปกรณ์อื่นออกและคงอุปกรณ์นี้ไว้
			</CardDescription>
		</CardHeader>
		<CardContent>
			<form class="space-y-4" onsubmit={changePassword}>
				<div class="space-y-2">
					<Label for="account-current-password">รหัสผ่านปัจจุบัน</Label>
					<Input
						id="account-current-password"
						type="password"
						autocomplete="current-password"
						bind:value={currentPassword}
						disabled={actionBusy}
					/>
				</div>
				<div class="space-y-2">
					<Label for="account-new-password">รหัสผ่านใหม่</Label>
					<Input
						id="account-new-password"
						type="password"
						autocomplete="new-password"
						bind:value={newPassword}
						disabled={actionBusy}
						minlength={8}
						maxlength={128}
					/>
					<p class="text-muted-foreground text-xs">ใช้รหัสผ่าน 8–128 ตัวอักษร</p>
				</div>
				<div class="space-y-2">
					<Label for="account-confirm-password">ยืนยันรหัสผ่านใหม่</Label>
					<Input
						id="account-confirm-password"
						type="password"
						autocomplete="new-password"
						bind:value={confirmPassword}
						disabled={actionBusy}
						minlength={8}
						maxlength={128}
					/>
				</div>
				<LoadingButton
					type="submit"
					class="w-full"
					disabled={actionBusy}
					loading={isChangingPassword}
					loadingLabel="กำลังเปลี่ยนรหัสผ่าน..."
				>
					<KeyRound class="h-4 w-4" />
					เปลี่ยนรหัสผ่าน
				</LoadingButton>
			</form>
		</CardContent>
	</Card>
</div>

<AlertDialog.Root bind:open={currentSessionDialogOpen}>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ออกจากระบบอุปกรณ์นี้หรือไม่</AlertDialog.Title>
			<AlertDialog.Description>
				คุณจะต้องเข้าสู่ระบบใหม่บนอุปกรณ์นี้ การดำเนินการนี้จะไม่กระทบอุปกรณ์อื่น
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={revokingSessionId !== null}>ยกเลิก</AlertDialog.Cancel>
			<LoadingButton
				variant="destructive"
				loading={revokingSessionId === selectedCurrentSession?.id}
				loadingLabel="กำลังออกจากระบบ..."
				onclick={revokeCurrentSession}
			>
				ออกจากระบบ
			</LoadingButton>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>

<AlertDialog.Root bind:open={logoutAllDialogOpen}>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>ออกจากระบบทุกอุปกรณ์หรือไม่</AlertDialog.Title>
			<AlertDialog.Description>
				ทุกอุปกรณ์รวมถึงอุปกรณ์นี้จะต้องเข้าสู่ระบบใหม่
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={isLoggingOutAll}>ยกเลิก</AlertDialog.Cancel>
			<LoadingButton
				variant="destructive"
				loading={isLoggingOutAll}
				loadingLabel="กำลังออกจากระบบ..."
				onclick={logoutAllSessions}
			>
				ออกจากระบบทุกอุปกรณ์
			</LoadingButton>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
