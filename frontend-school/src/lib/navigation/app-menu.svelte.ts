import { getContext, setContext } from 'svelte';
import { getUserMenu, type MenuGroup } from '#lib/api/menu.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { LatestRequest } from '#lib/async/latest-request.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';

export type AppMenuRead = { identityKey: string; groups: MenuGroup[] };
const contextKey = Symbol('app-menu-region');

/** Layout-owned region state, shared with the dashboard and Sidebar for this layout lifetime. */
class AppMenuRegion {
	groups = $state<MenuGroup[]>([]);
	loading = $state(true);
	loaded = $state(false);
	error = $state('');
	private request = new LatestRequest();

	consume(operation: Promise<RouteLoadResult<AppMenuRead>>): void {
		const ticket = this.request.begin();
		this.loading = true;
		this.error = '';
		void operation.then((result) => this.apply(result, ticket.revision));
	}
	private apply(result: RouteLoadResult<AppMenuRead>, revision: number): void {
		if (!this.request.isCurrent(revision)) return;
		this.loading = false;
		if (!result.ok) {
			this.error = result.error;
			return;
		}
		if (result.data.identityKey !== appIdentityKey()) return;
		this.groups = result.data.groups;
		this.loaded = true;
	}
	retry = async (): Promise<void> => {
		const user = await waitForAuthenticatedUser();
		if (!user) return;
		const identityKey = appIdentityKey();
		const ticket = this.request.begin();
		this.loading = true;
		this.error = '';
		const result = await captureRouteLoad(
			getUserMenu({ signal: ticket.signal }).then((response) => ({
				identityKey,
				groups: response.groups
			})),
			'โหลดเมนูบริการไม่สำเร็จ'
		);
		this.apply(result, ticket.revision);
	};
	reset(): void {
		this.request.abort();
		this.groups = [];
		this.loaded = false;
		this.loading = false;
		this.error = '';
	}
}

export function createAppMenuRegion() {
	return setContext(contextKey, new AppMenuRegion());
}
export function getAppMenuRegion() {
	return getContext<AppMenuRegion>(contextKey);
}
