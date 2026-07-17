import { readFileSync } from 'node:fs';

import { describe, expect, it, vi } from 'vitest';

import type {
  ApiErrorEnvelope,
  HttpRequestEnvelope,
  HttpSuccessEnvelope,
} from '../src/ui/shared/product/contracts/common';
import type {
  CatalogDiscoveryRequestV1,
  CatalogDiscoveryResponseV1,
} from '../src/ui/shared/product/contracts/catalog';
import type {
  CourseDetailRequestV1,
  CourseDetailResponseV1,
  CourseQueryRequestV1,
  CourseQueryResponseV1,
  FilterSchemaV1,
  SectionDetailRequestV1,
  SectionDetailResponseV1,
  SectionQueryRequestV1,
  SectionQueryResponseV1,
} from '../src/ui/shared/product/contracts/query';
import type {
  OpenRefreshStatusV1,
  OpenSectionStatusRequestV1,
  OpenSectionStatusV1,
  OpenStatusRequestV1,
} from '../src/ui/shared/product/contracts/open';
import { ProductApi } from '../src/ui/shared/product/ProductApi';
import { ProductClient, ProductClientError } from '../src/ui/shared/product/ProductClient';

function readGolden<T>(name: string): T {
  return JSON.parse(readFileSync(
    new URL(`../../crates/bcsp-contracts/tests/golden/${name}`, import.meta.url),
    'utf8',
  )) as T;
}

describe('ProductClient', () => {
  it('adds the current session only to a mutation and uses the v1 request envelope', async () => {
    const request = readGolden<HttpRequestEnvelope<unknown>>('http-request-v1.json');
    const success = readGolden<HttpSuccessEnvelope<unknown>>('http-success-v1.json');
    let session = 'session-one';
    const fetchMock = vi.fn<typeof fetch>(async () => Response.json(success));
    const client = new ProductClient({
      baseUrl: 'https://planner.invalid/',
      fetch: fetchMock,
      session: () => session,
    });

    await expect(client.post('/api/v1/query/courses', request.payload)).resolves.toEqual(success.data);
    const [url, init] = fetchMock.mock.calls[0] ?? [];
    expect(url).toBe('https://planner.invalid/api/v1/query/courses');
    expect(init?.credentials).toBe('same-origin');
    expect(new Headers(init?.headers).get('x-bcsp-session')).toBe('session-one');
    expect(JSON.parse(String(init?.body))).toEqual(request);

    session = 'session-two';
    await client.post('/api/v1/query/courses', request.payload);
    expect(new Headers(fetchMock.mock.calls[1]?.[1]?.headers).get('x-bcsp-session')).toBe('session-two');
  });

  it('keeps a read session-free and rejects cross-surface paths', async () => {
    const success = readGolden<HttpSuccessEnvelope<unknown>>('http-success-v1.json');
    const fetchMock = vi.fn<typeof fetch>(async () => Response.json(success));
    const client = new ProductClient({ fetch: fetchMock, session: () => 'secret-session' });
    await client.get('/api/v1/query/filter-schema');
    expect(new Headers(fetchMock.mock.calls[0]?.[1]?.headers).has('x-bcsp-session')).toBe(false);
    await expect(client.get('https://classes.rutgers.edu/api/catalog')).rejects.toThrow(
      /same-host \/api\/ paths/u,
    );
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('surfaces the Rust API error envelope without trusting a free-form body', async () => {
    const error = readGolden<ApiErrorEnvelope>('http-error-v1.json');
    const client = new ProductClient({
      fetch: vi.fn<typeof fetch>(async () => Response.json(error, { status: 400 })),
    });
    const result = client.post('/api/v1/query/courses', {});
    await expect(result).rejects.toBeInstanceOf(ProductClientError);
    await expect(result).rejects.toMatchObject({
      status: 400,
      message: 'error.selection_limit_exceeded',
      apiError: error,
    });
  });
});

describe('ProductApi', () => {
  it('authenticates only the scoped service-status read and preserves repeated Campus values', async () => {
    const success = readGolden<HttpSuccessEnvelope<unknown>>('http-success-v1.json');
    const fetchMock = vi.fn<typeof fetch>(async () => Response.json(success));
    const api = new ProductApi(new ProductClient({
      baseUrl: 'https://planner.invalid/',
      fetch: fetchMock,
      session: () => 'scope-session',
    }));

    await api.serviceStatus();
    await api.serviceStatus(undefined, { term: '72026', campuses: ['NB', 'NK'] });

    const [unscopedUrl, unscopedInit] = fetchMock.mock.calls[0] ?? [];
    expect(String(unscopedUrl)).toBe('https://planner.invalid/api/v1/service/status');
    expect(new Headers(unscopedInit?.headers).has('x-bcsp-session')).toBe(false);
    const [scopedUrl, scopedInit] = fetchMock.mock.calls[1] ?? [];
    const scoped = new URL(String(scopedUrl));
    expect(scoped.pathname).toBe('/api/v1/service/status');
    expect(scoped.searchParams.get('activeTerm')).toBe('72026');
    expect(scoped.searchParams.getAll('activeCampus')).toEqual(['NB', 'NK']);
    expect(new Headers(scopedInit?.headers).get('x-bcsp-session')).toBe('scope-session');
  });

  it('binds every typed operation to the eight shared Rust routes', async () => {
    const rustRequest = readGolden<HttpRequestEnvelope<{
      readonly sectionKey: { readonly term: string; readonly campus: string; readonly index: string };
      readonly courseGroupKey: {
        readonly term: string;
        readonly campus: string;
        readonly courseString: string;
      };
    }>>('http-request-v1.json');
    const courseRequest = readGolden<CourseQueryRequestV1>('course-query-request-v1.json');
    const sectionRequest: SectionQueryRequestV1 = {
      ...courseRequest,
      sort: { field: 'SECTION_INDEX', direction: 'ASCENDING' },
    };
    const discoveryRequest: CatalogDiscoveryRequestV1 = { contractVersion: 1 };
    const courseDetailRequest: CourseDetailRequestV1 = {
      contractVersion: 1,
      key: rustRequest.payload.courseGroupKey,
    };
    const sectionDetailRequest: SectionDetailRequestV1 = {
      contractVersion: 1,
      key: rustRequest.payload.sectionKey,
    };
    const openStatusRequest: OpenStatusRequestV1 = {
      contractVersion: 1,
      batch: {
        term: rustRequest.payload.sectionKey.term,
        campus: rustRequest.payload.sectionKey.campus,
      },
    };
    const openSectionStatusRequest: OpenSectionStatusRequestV1 = {
      contractVersion: 1,
      sectionKey: rustRequest.payload.sectionKey,
    };
    const rustSuccess = readGolden<HttpSuccessEnvelope<unknown>>('http-success-v1.json');
    const fetchMock = vi.fn<typeof fetch>(async () => Response.json(rustSuccess));
    const api = new ProductApi(new ProductClient({
      baseUrl: 'https://planner.invalid/',
      fetch: fetchMock,
      session: () => 'synthetic-session',
    }));

    const filterSchema: Promise<FilterSchemaV1> = api.filterSchema();
    const discovery: Promise<CatalogDiscoveryResponseV1> = api.catalogDiscovery(discoveryRequest);
    const courses: Promise<CourseQueryResponseV1> = api.searchCourses(courseRequest);
    const sections: Promise<SectionQueryResponseV1> = api.searchSections(sectionRequest);
    const courseDetail: Promise<CourseDetailResponseV1> = api.courseDetail(courseDetailRequest);
    const sectionDetail: Promise<SectionDetailResponseV1> = api.sectionDetail(sectionDetailRequest);
    const openStatus: Promise<OpenRefreshStatusV1> = api.openStatus(openStatusRequest);
    const openSectionStatus: Promise<OpenSectionStatusV1> = api.openSectionStatus(
      openSectionStatusRequest,
    );
    await Promise.all([
      filterSchema,
      discovery,
      courses,
      sections,
      courseDetail,
      sectionDetail,
      openStatus,
      openSectionStatus,
    ]);

    expect(fetchMock.mock.calls.map(([url, init]) => ({
      envelope: init?.body === undefined ? null : JSON.parse(String(init.body)) as unknown,
      method: init?.method,
      path: new URL(String(url)).pathname,
      session: new Headers(init?.headers).get('x-bcsp-session'),
    }))).toEqual([
      {
        envelope: null,
        method: 'GET',
        path: '/api/v1/query/filter-schema',
        session: null,
      },
      {
        envelope: { protocolVersion: 1, payload: discoveryRequest },
        method: 'POST',
        path: '/api/v1/catalog/discovery',
        session: 'synthetic-session',
      },
      {
        envelope: { protocolVersion: 1, payload: courseRequest },
        method: 'POST',
        path: '/api/v1/query/courses',
        session: 'synthetic-session',
      },
      {
        envelope: { protocolVersion: 1, payload: sectionRequest },
        method: 'POST',
        path: '/api/v1/query/sections',
        session: 'synthetic-session',
      },
      {
        envelope: { protocolVersion: 1, payload: courseDetailRequest },
        method: 'POST',
        path: '/api/v1/query/course-detail',
        session: 'synthetic-session',
      },
      {
        envelope: { protocolVersion: 1, payload: sectionDetailRequest },
        method: 'POST',
        path: '/api/v1/query/section-detail',
        session: 'synthetic-session',
      },
      {
        envelope: { protocolVersion: 1, payload: openStatusRequest },
        method: 'POST',
        path: '/api/v1/open/status',
        session: 'synthetic-session',
      },
      {
        envelope: { protocolVersion: 1, payload: openSectionStatusRequest },
        method: 'POST',
        path: '/api/v1/open/section-status',
        session: 'synthetic-session',
      },
    ]);
  });
});
