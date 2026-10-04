/** Event-driven reconciliation; registering handlers never starts an initial page read. */
export function registerDeliveryDraftReconcile(
	reconcile: () => void,
	needsPolling: () => boolean,
	targetWindow: Window = window,
	targetDocument: Document = document
): () => void {
	const visibleReconcile = () => {
		if (!targetDocument.hidden) reconcile();
	};
	const interval = targetWindow.setInterval(() => {
		if (needsPolling()) visibleReconcile();
	}, 30000);
	targetWindow.addEventListener('focus', visibleReconcile);
	targetDocument.addEventListener('visibilitychange', visibleReconcile);
	return () => {
		targetWindow.clearInterval(interval);
		targetWindow.removeEventListener('focus', visibleReconcile);
		targetDocument.removeEventListener('visibilitychange', visibleReconcile);
	};
}
