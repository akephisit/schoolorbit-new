export type DeploymentStatus = 'maintenance' | 'ready';

export interface DeploymentStatusResponse {
	status: DeploymentStatus;
	releaseId: string;
	retryAfterSeconds: number;
}

export interface DeploymentState {
	status: DeploymentStatus;
	releaseId: string | null;
}

interface DeploymentMonitorDependencies {
	readStatus: () => Promise<DeploymentStatusResponse>;
	schedule: (callback: () => void | Promise<void>, delay: number) => unknown;
	cancelSchedule: (handle: unknown) => void;
	reload: () => void;
	storage: Pick<Storage, 'getItem' | 'setItem'>;
	onVisible: (callback: () => void | Promise<void>) => () => void;
}

export interface DeploymentMonitor {
	subscribe: (listener: (state: DeploymentState) => void) => () => void;
	start: () => Promise<void>;
	stop: () => void;
	probe: () => Promise<void>;
	confirmMaintenance: () => Promise<void>;
}

const RELEASE_ID_PATTERN = /^[0-9a-f]{40}$/;
const POLL_INTERVAL_MS = 10_000;
const RELOAD_GUARD_PREFIX = 'schoolorbit:deployment-reloaded:';

function isRecord(value: unknown): value is Record<string, unknown> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

export function isMaintenanceResponse(status: number, payload: unknown): boolean {
	return (
		status === 503 &&
		isRecord(payload) &&
		payload.success === false &&
		payload.error === 'maintenance'
	);
}

export function parseDeploymentStatus(payload: unknown): DeploymentStatusResponse | null {
	if (!isRecord(payload)) return null;
	if (payload.status !== 'maintenance' && payload.status !== 'ready') return null;
	if (typeof payload.releaseId !== 'string' || !RELEASE_ID_PATTERN.test(payload.releaseId)) {
		return null;
	}
	if (payload.retryAfterSeconds !== 10) return null;
	return {
		status: payload.status,
		releaseId: payload.releaseId,
		retryAfterSeconds: payload.retryAfterSeconds
	};
}

export function createDeploymentMonitor(
	dependencies: DeploymentMonitorDependencies
): DeploymentMonitor {
	let state: DeploymentState = { status: 'ready', releaseId: null };
	let maintenanceConfirmed = false;
	let timer: unknown;
	let removeVisibilityListener: (() => void) | undefined;
	let started = false;
	let checking = false;
	let lifecycleGeneration = 0;
	const listeners = new Set<(next: DeploymentState) => void>();

	const publish = (next: DeploymentState) => {
		state = next;
		for (const listener of listeners) listener(state);
	};

	const clearPoll = () => {
		if (timer === undefined) return;
		dependencies.cancelSchedule(timer);
		timer = undefined;
	};

	const schedulePoll = (generation: number) => {
		if (!started || generation !== lifecycleGeneration) return;
		timer = dependencies.schedule(checkStatus, POLL_INTERVAL_MS);
	};

	const reloadOnce = (releaseId: string) => {
		const guardKey = `${RELOAD_GUARD_PREFIX}${releaseId}`;
		if (dependencies.storage.getItem(guardKey) === '1') return false;
		dependencies.storage.setItem(guardKey, '1');
		dependencies.reload();
		return true;
	};

	const checkStatus = async () => {
		if (checking) return;
		const generation = lifecycleGeneration;
		checking = true;
		clearPoll();
		try {
			const result = await dependencies.readStatus();
			if (generation !== lifecycleGeneration) return;
			if (result.status === 'maintenance') {
				if (state.status !== 'maintenance' || state.releaseId !== result.releaseId) {
					dependencies.storage.setItem(`${RELOAD_GUARD_PREFIX}${result.releaseId}`, '0');
				}
				maintenanceConfirmed = true;
				publish({ status: 'maintenance', releaseId: result.releaseId });
				schedulePoll(generation);
				return;
			}

			if (!maintenanceConfirmed) {
				if (
					state.releaseId !== null &&
					state.releaseId !== result.releaseId &&
					reloadOnce(result.releaseId)
				) {
					return;
				}
				publish({ status: 'ready', releaseId: result.releaseId });
				return;
			}

			if (reloadOnce(result.releaseId)) return;

			maintenanceConfirmed = false;
			publish({ status: 'ready', releaseId: result.releaseId });
		} catch {
			if (generation === lifecycleGeneration && maintenanceConfirmed) {
				schedulePoll(generation);
			}
		} finally {
			if (generation === lifecycleGeneration) checking = false;
		}
	};

	return {
		subscribe(listener) {
			listeners.add(listener);
			listener(state);
			return () => listeners.delete(listener);
		},
		async start() {
			if (started) return;
			started = true;
			lifecycleGeneration += 1;
			checking = false;
			removeVisibilityListener = dependencies.onVisible(checkStatus);
			await checkStatus();
		},
		stop() {
			started = false;
			lifecycleGeneration += 1;
			checking = false;
			clearPoll();
			removeVisibilityListener?.();
			removeVisibilityListener = undefined;
		},
		probe: checkStatus,
		async confirmMaintenance() {
			maintenanceConfirmed = true;
			publish({ status: 'maintenance', releaseId: state.releaseId });
			await checkStatus();
		}
	};
}
