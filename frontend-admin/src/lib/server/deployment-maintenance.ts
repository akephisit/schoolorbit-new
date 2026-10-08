export async function deploymentIsInMaintenance(api: string, fetcher = fetch): Promise<boolean> {
	try {
		const response = await fetcher(new URL('/deployment-status', api), {
			cache: 'no-store',
			signal: AbortSignal.timeout(3000)
		});
		if (!response.ok) return false;
		const body: unknown = await response.json();
		return (
			typeof body === 'object' && body !== null && 'status' in body && body.status === 'maintenance'
		);
	} catch {
		// A failed status probe alone does not establish deployment maintenance.
		return false;
	}
}

export function maintenanceResponse(api: string): Response {
	const endpoint = JSON.stringify(new URL('/deployment-status', api).href).replaceAll(
		'<',
		'\\u003c'
	);
	return new Response(
		`<!doctype html><html lang="th"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>กำลังปรับปรุงระบบ</title><body><main style="max-width:36rem;margin:15vh auto;padding:1.5rem;font-family:system-ui;text-align:center"><h1>กำลังปรับปรุงระบบ</h1><p>กรุณารอสักครู่ ระบบจะกลับมาให้บริการเมื่อปรับปรุงเสร็จ</p></main><script>setInterval(async()=>{if(document.hidden)return;try{const r=await fetch(${endpoint},{cache:'no-store'});if(r.ok&&(await r.json()).status==='ready')location.reload()}catch{}},10000)</script></body></html>`,
		{
			status: 503,
			headers: {
				'Content-Type': 'text/html; charset=utf-8',
				'Cache-Control': 'no-store',
				'Retry-After': '10'
			}
		}
	);
}
