(function () {
	var root = document.documentElement;
	var startedAt = performance.now();
	// Adapt the existing personal preference before any UI or loader paints.
	var preference = 'system';
	try {
		var stored = JSON.parse(localStorage.getItem('ui-preferences') || 'null');
		if (
			stored &&
			(stored.theme === 'dark' || stored.theme === 'light' || stored.theme === 'system')
		)
			preference = stored.theme;
	} catch (_) {
		/* Use the existing system default when storage is unavailable. */
	}
	var publicPage = location.pathname === '/' || location.pathname.indexOf('/login') === 0;
	root.classList.toggle(
		'dark',
		!publicPage &&
			(preference === 'dark' ||
				(preference === 'system' && matchMedia('(prefers-color-scheme: dark)').matches))
	);
	var chrome = document.querySelector('meta[name="theme-color"]');
	if (chrome)
		chrome.setAttribute('content', getComputedStyle(root).getPropertyValue('--background').trim());
	var selected = getComputedStyle(root).getPropertyValue('--school-font-family').trim();
	var finished = false;
	function reveal(fallback) {
		if (finished) return;
		finished = true;
		fallback = fallback || performance.now() - startedAt >= 3000;
		if (fallback) {
			root.style.setProperty('--school-font-family', 'system-ui, sans-serif');
			root.dataset.schoolFontFallback = 'true';
		}
		root.dataset.schoolFontPending = 'false';
	}
	var timer = setTimeout(function () {
		reveal(true);
	}, 3000);
	if (!document.fonts || !selected) {
		clearTimeout(timer);
		reveal(true);
		return;
	}
	var family = selected.split(',')[0];
	Promise.all(
		[400, 500, 600, 700].map(function (weight) {
			return document.fonts
				.load(weight + ' 16px ' + family, 'กขคง SchoolOrbit 0123456789')
				.then(function (faces) {
					if (!faces.length) throw new Error('School font unavailable');
				});
		})
	).then(
		function () {
			clearTimeout(timer);
			reveal(false);
		},
		function () {
			clearTimeout(timer);
			reveal(true);
		}
	);
})();
