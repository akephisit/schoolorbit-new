/** Reconcile live Delivery on user return without introducing business state or polling. */
export function registerDeliveryReconcile(
	reconcile: () => void,
	targetWindow: Window = window,
	targetDocument: Document = document
): () => void {
	const visible = () => {
		if (!targetDocument.hidden) reconcile();
	};
	targetWindow.addEventListener('focus', visible);
	targetDocument.addEventListener('visibilitychange', visible);
	return () => {
		targetWindow.removeEventListener('focus', visible);
		targetDocument.removeEventListener('visibilitychange', visible);
	};
}
