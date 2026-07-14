// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { cleanup, render } from '@testing-library/react';
import { createPortal } from 'react-dom';
import { afterEach, describe, expect, it } from 'vitest';

import { PublicCompositionRoot } from '../src/ui/public/PublicCompositionRoot';

interface DenyCapability {
  readonly capability: string;
  readonly markers: readonly string[];
}

const denyCapabilities = (JSON.parse(
  readFileSync(
    resolve(process.cwd(), '../tools/architecture/p4-public-source-deny.json'),
    'utf8',
  ),
) as { readonly capabilities: readonly DenyCapability[] }).capabilities;
const supplementalMarkers: Readonly<Record<string, readonly string[]>> = {
  LOCAL_RESET: ['local_user_data_reset'],
};
const productBootstrap = {
  protocolVersion: 1,
  data: { sessionNonce: '10000000-0000-4000-8000-000000000001' },
};

function normalize(value: string): string {
  return value.toLocaleLowerCase('en-US').replace(/[^a-z0-9]/gu, '');
}

function publicDomViolations(root: ParentNode) {
  const renderedDom = normalize(
    [...root.querySelectorAll('*')]
      .flatMap((element) => [
        element.textContent ?? '',
        ...[...element.attributes].flatMap(({ name, value }) => [name, value]),
      ])
      .join('\n'),
  );
  return denyCapabilities.flatMap(({ capability, markers }) =>
    [...markers, ...(supplementalMarkers[capability] ?? [])]
      .filter((marker) => renderedDom.includes(normalize(marker)))
      .map((marker) => ({ capability, marker })),
  );
}

afterEach(() => {
  cleanup();
  document.documentElement.lang = '';
});

function HiddenPortalFixture() {
  return createPortal(
    <button type="button" hidden aria-label="Local user data reset" />,
    document.body,
  );
}

describe('public rendered DOM zero surface', () => {
  it.each(['en-US', 'zh-CN'])(
    'contains no excluded text or attributes across the full %s document',
    (locale) => {
      const { baseElement } = render(
        <PublicCompositionRoot
          localeSource={{ languages: [locale] }}
          product={{ bootstrap: productBootstrap }}
        />,
      );
      expect(
        baseElement.querySelector('[data-bcsp-product-state]')?.getAttribute(
          'data-bcsp-product-state',
        ),
      ).toBe('READY');
      expect(publicDomViolations(baseElement.ownerDocument)).toEqual([]);
    },
  );

  it('detects an excluded hidden portal outside the render container', () => {
    const { baseElement, container } = render(
      <>
        <PublicCompositionRoot localeSource={{ languages: ['en-US'] }} />
        <HiddenPortalFixture />
      </>,
    );
    const hiddenControl = baseElement.querySelector('[aria-label="Local user data reset"]');
    if (hiddenControl === null) throw new Error('portal fixture was not rendered');
    expect(container.contains(hiddenControl)).toBe(false);

    expect(publicDomViolations(baseElement.ownerDocument)).toContainEqual({
      capability: 'LOCAL_RESET',
      marker: 'local_user_data_reset',
    });
  });
});
