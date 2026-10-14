import { appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const SCHOOL_BUILD_CACHE_PACKAGE = 'schoolorbit-backend-school-cache';
const OWNER = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/;

export function backendBuildCache({ component, owner, ref }) {
  if (!['backend-admin', 'backend-school'].includes(component)) throw new Error('Unknown backend cache owner');
  if (typeof owner !== 'string' || !OWNER.test(owner)) throw new Error('Invalid registry owner');
  if (typeof ref !== 'string' || !ref || /[\r\n]/.test(ref)) throw new Error('Invalid build ref');
  if (component === 'backend-admin') {
    const from = 'type=gha,scope=backend-admin';
    return { from, to: `${from},mode=max` };
  }
  const reference = `ghcr.io/${owner.toLowerCase()}/${SCHOOL_BUILD_CACHE_PACKAGE}:buildcache`;
  const from = `type=registry,ref=${reference}`;
  return { from, to: ref === 'refs/heads/main' ? `${from},mode=max,image-manifest=true,oci-mediatypes=true` : '' };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const cache = backendBuildCache({ component: process.env.COMPONENT, owner: process.env.GITHUB_REPOSITORY_OWNER, ref: process.env.GITHUB_REF });
  if (!process.env.GITHUB_OUTPUT) throw new Error('GitHub output path is unavailable');
  appendFileSync(process.env.GITHUB_OUTPUT, `from=${cache.from}\nto=${cache.to}\n`);
}
