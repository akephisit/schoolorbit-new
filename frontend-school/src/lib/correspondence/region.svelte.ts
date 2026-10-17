import { LatestRequest } from '#lib/async/latest-request.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';

/** One correspondence region; route loaders supply its first read, interactions own retries. */
export class CorrespondenceRegion<T> {
	value = $state<T | null>(null);
	loading = $state(true);
	error = $state('');
	private request = new LatestRequest();
	private disposed = false;
	consume(operation: Promise<RouteLoadResult<T>>) {
		const ticket = this.request.begin();
		this.loading = true;
		this.error = '';
		void operation.then((result) => {
			if (!this.request.isCurrent(ticket.revision) || this.disposed) return;
			this.loading = false;
			if (result.ok) this.value = result.data;
			else this.error = result.error;
		});
	}
	retry(read: (signal: AbortSignal) => Promise<T>) {
		const ticket = this.request.begin();
		this.loading = true;
		this.error = '';
		void captureRouteLoad(read(ticket.signal), 'โหลดข้อมูลไม่สำเร็จ').then((result) => {
			if (!this.request.isCurrent(ticket.revision) || this.disposed) return;
			this.loading = false;
			if (result.ok) this.value = result.data;
			else this.error = result.error;
		});
	}
	patch(value: T) {
		if (!this.disposed) {
			this.request.abort();
			this.value = value;
			this.loading = false;
			this.error = '';
		}
	}
	dispose() {
		this.disposed = true;
		this.request.abort();
		this.value = null;
	}
}
