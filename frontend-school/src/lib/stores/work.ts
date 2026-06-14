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
	const { subscribe, set, update } = writable<WorkStoreState>(initialState);

	return {
		subscribe,

		async fetchCounts(options: { silent?: boolean } = {}) {
			if (!options.silent) {
				update((state) => ({ ...state, loadingCounts: true, error: null }));
			}

			try {
				const counts = await getMyWorkCounts();
				update((state) => ({
					...state,
					counts,
					loadingCounts: false,
					error: null
				}));
			} catch (error) {
				update((state) => ({
					...state,
					loadingCounts: false,
					error: error instanceof Error ? error.message : 'ไม่สามารถโหลดจำนวนงานได้'
				}));
			}
		},

		async fetchItems(options: { silent?: boolean } = {}) {
			if (!options.silent) {
				update((state) => ({ ...state, loadingItems: true, error: null }));
			}

			try {
				const items = await getMyWorkItems();
				update((state) => ({
					...state,
					items,
					loadingItems: false,
					error: null
				}));
			} catch (error) {
				update((state) => ({
					...state,
					loadingItems: false,
					error: error instanceof Error ? error.message : 'ไม่สามารถโหลดรายการงานได้'
				}));
			}
		},

		async refreshSilently() {
			await Promise.all([this.fetchCounts({ silent: true }), this.fetchItems({ silent: true })]);
		},

		reset() {
			set(initialState);
		}
	};
}

export const workStore = createWorkStore();
