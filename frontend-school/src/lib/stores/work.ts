import { getMyWorkCounts, getMyWorkItems, type WorkItem, type WorkItemCounts } from '$lib/api/work';
import { writable } from 'svelte/store';

interface WorkStoreState {
	items: WorkItem[];
	counts: WorkItemCounts;
	loadingItems: boolean;
	loadingCounts: boolean;
	error: string | null;
}

const emptyCounts: WorkItemCounts = {
	open: 0,
	dueSoon: 0,
	overdue: 0,
	submitted: 0,
	closed: 0,
	total: 0
};

const initialState: WorkStoreState = {
	items: [],
	counts: emptyCounts,
	loadingItems: false,
	loadingCounts: false,
	error: null
};

function createWorkStore() {
	let generation = 0;
	let countsRequest = 0;
	let itemsRequest = 0;
	const { subscribe, set, update } = writable<WorkStoreState>(initialState);

	return {
		subscribe,

		async fetchCounts(options: { silent?: boolean; isCurrent?: () => boolean } = {}) {
			const requestGeneration = generation;
			const request = ++countsRequest;
			const isCurrent = () =>
				requestGeneration === generation &&
				request === countsRequest &&
				(options.isCurrent?.() ?? true);
			if (!options.silent) {
				update((state) => ({ ...state, loadingCounts: true, error: null }));
			}

			try {
				const counts = await getMyWorkCounts();
				if (!isCurrent()) return false;
				update((state) => ({
					...state,
					counts,
					loadingCounts: false,
					error: null
				}));
				return true;
			} catch (error) {
				if (!isCurrent()) return false;
				update((state) => ({
					...state,
					loadingCounts: false,
					error: error instanceof Error ? error.message : 'ไม่สามารถโหลดจำนวนงานได้'
				}));
				return false;
			}
		},

		async fetchItems(options: { silent?: boolean; isCurrent?: () => boolean } = {}) {
			const requestGeneration = generation;
			const request = ++itemsRequest;
			const isCurrent = () =>
				requestGeneration === generation &&
				request === itemsRequest &&
				(options.isCurrent?.() ?? true);
			if (!options.silent) {
				update((state) => ({ ...state, loadingItems: true, error: null }));
			}

			try {
				const items = await getMyWorkItems();
				if (!isCurrent()) return false;
				update((state) => ({
					...state,
					items,
					loadingItems: false,
					error: null
				}));
				return true;
			} catch (error) {
				if (!isCurrent()) return false;
				update((state) => ({
					...state,
					loadingItems: false,
					error: error instanceof Error ? error.message : 'ไม่สามารถโหลดรายการงานได้'
				}));
				return false;
			}
		},

		async refreshSilently(options: { isCurrent?: () => boolean } = {}) {
			const results = await Promise.all([
				this.fetchCounts({ silent: true, ...options }),
				this.fetchItems({ silent: true, ...options })
			]);
			return results.every(Boolean);
		},

		reset() {
			generation++;
			set(initialState);
		}
	};
}

export const workStore = createWorkStore();
