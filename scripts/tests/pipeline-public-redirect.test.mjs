import assert from 'node:assert/strict';
import test from 'node:test';
import {verifyCanonicalWwwRedirect} from '../lib/pipeline-public-redirect.mjs';

test('www accepts only permanent direct or HTTPS-then-apex redirects', async () => {
  for (const edgeHttps of [false, true]) {
    const calls = [];
    await verifyCanonicalWwwRedirect('https://example.test/', async (url, options) => {
      assert.equal(options.redirect, 'manual');
      assert(options.signal);
      calls.push(url.href);
      return new Response(null, {status:301, headers:{location:edgeHttps && url.protocol === 'http:' ? 'https://www.example.test/' : 'https://example.test/'}});
    });
    assert.equal(calls.length, edgeHttps ? 3 : 2);
    assert(calls.every(url => new URL(url).hostname === 'www.example.test'));
  }
});

test('www errors, loops, temporary and external redirects fail closed before acceptance', async () => {
  for (const [status, location] of [[526, null],[200,null],[302,'https://example.test/'],[301,'https://other.example.test/'],[301,'https://www.example.test/'],[301,'http://example.test/']]) {
    await assert.rejects(verifyCanonicalWwwRedirect('https://example.test/', async () => new Response(null,{status,headers:location ? {location} : {}})));
  }
  await assert.rejects(verifyCanonicalWwwRedirect('https://example.test/', async () => {throw new Error('offline');}));
  await assert.rejects(verifyCanonicalWwwRedirect('http://example.test/', () => assert.fail('Unexpected request')));
});
