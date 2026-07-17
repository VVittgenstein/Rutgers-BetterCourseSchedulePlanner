if (typeof navigator !== 'undefined' && navigator.userAgent.toLowerCase().includes('jsdom')) {
  Object.defineProperty(globalThis, 'scrollTo', {
    configurable: true,
    value: () => undefined,
    writable: true,
  });
}
