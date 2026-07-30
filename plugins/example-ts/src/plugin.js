// A reference provider plugin in JavaScript.
//
// This exists to make one claim honest: **the ABI is language-agnostic.** The Rust plugin next
// door could be mistaken for a special case — same language as the host, same toolchain, maybe
// some shared assumption leaking through. This one shares nothing with the host but the WIT file,
// and the conformance suite runs the *same* assertions against both.
//
// Written in plain JavaScript rather than TypeScript on purpose. `jco componentize` takes JS
// directly, so a TS build step would add a compiler to the story without adding anything to the
// demonstration — and a plugin author should be able to see the whole path from source to
// component without one.
//
// Two things to notice, both consequences of the host lending `fetch`:
//
//   1. There is no `node:https`, no `fetch` polyfill, no dependency of any kind. The runtime here
//      has no sockets to offer, so the host does the networking and this file parses bytes.
//   2. Errors are *thrown*, not returned. That is jco's mapping for a WIT `result`: the error arm
//      of `provider-error` becomes a thrown variant object, and the host receives it as the same
//      typed error a Rust plugin would return.

// The version is part of the specifier: the WIT package is `anistream:provider@1.1.0`, and jco
// resolves imports against the fully-qualified interface name.
import {
  fetch,
  log,
  aesDecrypt,
  regexCaptures,
  configGet,
} from 'anistream:provider/host@1.1.0';

// The one host this plugin may reach. Declared here, enforced by the host — visible in
// `anistream --plugins` without reading this file.
const ALLOWED_HOST = 'httpbin.org';

/// `https://cdn.example.test/master.m3u8` under AES-128-CBC, with the key and iv below.
///
/// The same ciphertext the Rust plugin carries, decrypted by the same host function — which is the
/// point: two unrelated guests, one implementation of the capability.
const SEALED_URL = new Uint8Array([
  0x98, 0xe2, 0x0c, 0xda, 0xa0, 0x6e, 0xce, 0x05, 0xbb, 0x5f, 0x6f, 0x31, 0x13, 0x71, 0xa4, 0xe2,
  0x91, 0xfd, 0xbc, 0xc7, 0xbb, 0xc5, 0xf0, 0xa6, 0xfb, 0x55, 0x0d, 0xfc, 0x58, 0xff, 0x26, 0x9e,
  0x26, 0x74, 0x61, 0x04, 0xfb, 0x4b, 0x14, 0xb6, 0x68, 0xcc, 0xb0, 0x28, 0x5c, 0x24, 0x15, 0x0a,
]);

const AES_KEY = new TextEncoder().encode('anistream-demo!!');
const AES_IV = new TextEncoder().encode('0123456789abcdef');

/// Percent-encode a query value. Hand-rolled for the same reason as in the Rust plugin: pulling a
/// dependency in to escape one parameter would be most of the component.
function encode(value) {
  return [...new TextEncoder().encode(value)]
    .map((byte) => {
      const char = String.fromCharCode(byte);
      return /[A-Za-z0-9\-_.~]/.test(char)
        ? char
        : `%${byte.toString(16).padStart(2, '0').toUpperCase()}`;
    })
    .join('');
}

/// A GET through the host, with the host's failures translated into ours.
///
/// The host's errors arrive as thrown variant objects too, so this is a `catch` rather than a
/// return-value check.
function get(url) {
  let response;
  try {
    response = fetch({
      method: 'GET',
      url,
      headers: [['accept', 'application/json']],
      body: undefined,
    });
  } catch (error) {
    // A denial means the manifest and this code disagree — a bug here, not a site being down, so
    // it is reported as `other` rather than `blocked`.
    if (error?.payload?.tag === 'denied') {
      throw { tag: 'other', val: `denied by the host: ${error.payload.val}` };
    }
    if (error?.payload?.tag === 'timeout') {
      throw { tag: 'blocked', val: 'timed out' };
    }
    throw { tag: 'blocked', val: String(error?.payload?.val ?? error) };
  }

  if (response.status >= 200 && response.status <= 299) {
    return new TextDecoder().decode(new Uint8Array(response.body));
  }
  if (response.status === 404) {
    throw { tag: 'not-found' };
  }
  // The signature source failure, and the one the registry fails over on.
  if (response.status === 403 || response.status === 429) {
    throw { tag: 'blocked', val: `status ${response.status}` };
  }
  throw { tag: 'other', val: `unexpected status ${response.status}` };
}

export const provider = {
  describe() {
    return {
      id: 'example-ts',
      displayName: 'Example (JavaScript)',
      version: '0.1.0',
      allowedHosts: [ALLOWED_HOST],
      translationTypes: ['sub', 'dub'],
      // Open strings a future host can gate optional treatment on. Nothing to declare — the
      // normal case, and what keeps declaring one meaningful.
      capabilities: [],
    };
  },

  search(query, translation) {
    log('debug', `search ${JSON.stringify(query)} (${translation})`);

    const body = get(`https://${ALLOWED_HOST}/get?q=${encode(query)}`);

    // The lent regex. A component bundling its own engine would be an order of magnitude larger
    // than this whole file.
    const matches = regexCaptures('"q":\\s*"([^"]*)"', body);
    if (matches.length === 0 || matches[0].length < 2) {
      throw { tag: 'parse', val: 'no query echoed back' };
    }
    const echoed = matches[0][1];

    return [
      {
        id: `example:${echoed}`,
        title: `Echo of ${echoed}`,
        // Alternate spellings widen the match surface; sources romanise inconsistently.
        synonyms: [`${echoed} (echo)`],
        episodeCount: 3,
        year: 2026,
        // A match gate, not a label: `tv` stops a series search landing on a movie.
        format: 'tv',
      },
    ];
  },

  listEpisodes(id, _translation) {
    // `not-found` rather than an empty list: the distinction is load-bearing in the host, where
    // `not-found` deliberately does *not* trigger failover to the next provider.
    if (!id.startsWith('example:')) {
      throw { tag: 'not-found' };
    }
    const slug = id.slice('example:'.length);
    log('debug', `episodes for ${slug}`);

    return [1, 2, 3].map((n) => ({
      number: String(n),
      title: `${slug} — part ${n}`,
      durationSecs: 1440,
      description: `In which ${slug} is echoed for the ${n}th time, and nothing else happens.`,
      thumbnail: undefined,
      airDate: `2026-01-${String(n).padStart(2, '0')}`,
      // `undefined` would mean "no claim"; this catalogue positively claims canon.
      filler: false,
    }));
  },

  resolve(id, episode, translation) {
    return resolveStreams(id, episode, translation);
  },

  // The selectable releases for an episode, for the Sources overlay. Two candidates so the
  // overlay has an actual choice; a source that resolves to exactly one stream should return
  // an empty list instead — that is the honest answer, not an error.
  sources(id, episode, _translation) {
    if (!id.startsWith('example:')) {
      throw { tag: 'not-found' };
    }
    const slug = id.slice('example:'.length);
    return [1080, 720].map((quality) => ({
      id: `${slug}:${episode}:${quality}`,
      title: `[Echo] ${slug} - ${episode} (${quality}p)`,
      quality,
      seeders: undefined,
      size: quality === 1080 ? '1.4 GiB' : '700 MiB',
      dualAudio: false,
      dubbed: false,
    }));
  },

  // Resolve one candidate from `sources` by its id. Never falls back to the automatic pick —
  // substituting another stream for the one the user chose would silently undo the choice.
  resolveSource(id, episode, translation, sourceId) {
    const quality = Number(sourceId.split(':').pop());
    if (!Number.isFinite(quality)) {
      throw { tag: 'not-found' };
    }
    return resolveStreams(id, episode, translation).map((stream) => ({ ...stream, quality }));
  },
};

// Shared by `resolve` and `resolveSource` as a plain function: jco invokes exports without a
// receiver, so `this.resolve(…)` inside the export object traps at runtime.
function resolveStreams(id, episode, translation) {
  if (!id.startsWith('example:')) {
    throw { tag: 'not-found' };
  }
  const slug = id.slice('example:'.length);

  // The lent AES: this component carries no crypto at all, yet decrypts a real payload.
  let url;
  try {
    url = new TextDecoder().decode(new Uint8Array(aesDecrypt(AES_KEY, AES_IV, SEALED_URL)));
  } catch (error) {
    throw { tag: 'parse', val: `aes: ${error?.payload ?? error}` };
  }

  // The lent settings: an optional mirror override from
  // `[providers.plugins.settings.example-ts]`. The fallback is the pattern to copy — a
  // plugin must work with every setting unset, because most users never write any.
  const cdn = configGet('cdn') ?? 'cdn.example.test';

  log('info', `resolved ${slug} ep ${episode} (${translation}) via ${cdn}`);

  return [
    {
      url: url.replace('cdn.example.test', cdn),
      kind: 'hls',
      quality: 1080,
      // Referer-locked CDNs 403 without this, which is why headers travel with the stream
      // rather than being the player's guess.
      headers: [['referer', `https://${ALLOWED_HOST}/`]],
      subtitles: [
        {
          language: 'eng',
          url: `https://${ALLOWED_HOST}/anything/${slug}.vtt`,
          hard: false,
          // The URL happens to end in `.vtt`; stating it anyway shows the field a source
          // with API-shaped subtitle URLs must set, or the player has to guess.
          format: 'vtt',
        },
      ],
      // This source streams only. Absent is how the download queue knows to say so.
      downloadSource: undefined,
      pickNote: undefined,
    },
  ];
}
