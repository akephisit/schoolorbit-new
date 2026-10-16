export async function verifyCanonicalWwwRedirect(origin, fetchImpl = fetch) {
  const apex = new URL(origin);
  if (apex.protocol !== 'https:' || apex.username || apex.password || apex.pathname !== '/' || apex.search || apex.hash) {
    throw new Error('Canonical redirect requires an HTTPS apex origin');
  }
  for (const protocol of ['http:', 'https:']) {
    const www = new URL(apex);
    www.hostname = `www.${apex.hostname}`;
    www.protocol = protocol;
    let current = www;
    for (let hop = 0; hop < 2; hop++) {
      const response = await fetchImpl(current, {redirect: 'manual', cache: 'no-store', signal: AbortSignal.timeout(15000)});
      const location = response.headers.get('location');
      if (![301, 308].includes(response.status) || !location) throw new Error('www must permanently redirect to the HTTPS apex');
      const target = new URL(location, current);
      if (target.href === apex.href) break;
      // Cloudflare Always Use HTTPS may precede the Worker's canonical redirect.
      if (hop === 0 && current.protocol === 'http:' && target.href === `https://${www.host}/`) {
        current = target;
        continue;
      }
      throw new Error('Unexpected www canonical redirect destination');
    }
  }
}
