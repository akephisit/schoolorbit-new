import { browser } from '$app/environment';
import { PUBLIC_BACKEND_URL } from '$env/static/public';
import { readable } from 'svelte/store';

import {
	createDeploymentMonitor,
	parseDeploymentStatus,
	type DeploymentState
} from './maintenance-controller';

const backendUrl = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';
const STATUS_REQUEST_TIMEOUT_MS = 5_000;
const initialState: DeploymentState = { status: 'ready', releaseId: null };
const serverStorage: Pick<Storage, 'getItem' | 'setItem'> = {
	getItem: () => null,
	setItem: () => {}
};

const monitor = createDeploymentMonitor({
	async readStatus() {
		const controller = new AbortController();
		const timeout = setTimeout(() => controller.abort(), STATUS_REQUEST_TIMEOUT_MS);
		try {
			const response = await fetch(`${backendUrl}/deployment-status`, {
				method: 'GET',
				credentials: 'omit',
				referrerPolicy: 'no-referrer',
				cache: 'no-store',
				signal: controller.signal,
				headers: { Accept: 'application/json' }
			});
			if (!response.ok) throw new Error('deployment_status_unavailable');
			const status = parseDeploymentStatus(await response.json());
			if (!status) throw new Error('deployment_status_invalid');
			return status;
		} finally {
			clearTimeout(timeout);
		}
	},
	schedule: (callback, delay) => setTimeout(() => void callback(), delay),
	cancelSchedule: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
	reload: () => {
		if (browser) window.location.reload();
	},
	storage: browser ? window.sessionStorage : serverStorage,
	onVisible: (callback) => {
		if (!browser) return () => {};
		const listener = () => {
			if (document.visibilityState === 'visible') void callback();
		};
		document.addEventListener('visibilitychange', listener);
		return () => document.removeEventListener('visibilitychange', listener);
	}
});

export const deploymentState = readable(initialState, (set) => monitor.subscribe(set));

export const startDeploymentMonitor = () => monitor.start();
export const stopDeploymentMonitor = () => monitor.stop();
export const probeDeploymentStatus = () => monitor.probe();
export const confirmMaintenance = () => monitor.confirmMaintenance();
