type WorkerContainer = Pick<ServiceWorkerContainer, 'register' | 'ready'>;

/** One registration owner for page startup and push, with bounded, retryable readiness. */
export function createServiceWorkerRegistrationOwner(
	container: WorkerContainer,
	timeoutMs = 10_000
): () => Promise<ServiceWorkerRegistration> {
	let registrationPromise: Promise<ServiceWorkerRegistration> | null = null;
	return () => {
		if (registrationPromise) return registrationPromise;
		let deadline: ReturnType<typeof setTimeout>;
		const work = container
			.register('/service-worker.js', { type: 'module', scope: '/', updateViaCache: 'none' })
			.then(() => container.ready);
		const timeout = new Promise<never>((_, reject) => {
			deadline = setTimeout(
				() => reject(new Error('ระบบแจ้งเตือนยังไม่พร้อม กรุณาลองอีกครั้ง')),
				timeoutMs
			);
		});
		const pending = Promise.race([work, timeout]).finally(() => clearTimeout(deadline));
		registrationPromise = pending;
		void pending.catch(() => {
			if (registrationPromise === pending) registrationPromise = null;
		});
		return pending;
	};
}

let owner: ReturnType<typeof createServiceWorkerRegistrationOwner> | null = null;

export function getServiceWorkerRegistration(): Promise<ServiceWorkerRegistration> {
	if (typeof navigator === 'undefined' || !('serviceWorker' in navigator)) {
		return Promise.reject(new Error('เบราว์เซอร์นี้ไม่รองรับระบบแจ้งเตือน'));
	}
	owner ??= createServiceWorkerRegistrationOwner(navigator.serviceWorker);
	return owner();
}
