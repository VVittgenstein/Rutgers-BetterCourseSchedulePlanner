import type { CatalogDiscoveryRequestV1, CatalogDiscoveryResponseV1 } from './contracts/catalog';
import type { TermCampusKey } from './contracts/common';
import type {
  CourseDetailRequestV1,
  CourseDetailResponseV1,
  CourseQueryRequestV1,
  CourseQueryResponseV1,
  DynamicFilterValidationRequestV3,
  DynamicFilterValidationResponseV3,
  FilterSchemaV1,
  FilterOptionsRequestV2,
  FilterOptionsResponseV2,
  SectionDetailRequestV1,
  SectionDetailResponseV1,
  SectionQueryRequestV1,
  SectionQueryResponseV1,
} from './contracts/query';
import type {
  OpenRefreshStatusV1,
  OpenSectionStatusRequestV1,
  OpenSectionStatusV1,
  OpenStatusRequestV1,
} from './contracts/open';
import type { ServiceStatus } from './contracts/service';
import type { ProductClientPort } from './ProductClient';

export const PRODUCT_API_ROUTES = {
  filterSchema: { method: 'GET', path: '/api/v1/query/filter-schema' },
  filterOptions: { method: 'POST', path: '/api/v1/query/filter-options' },
  validateDynamicFilters: { method: 'POST', path: '/api/v1/query/validate-dynamic-filters' },
  serviceStatus: { method: 'GET', path: '/api/v1/service/status' },
  catalogDiscovery: { method: 'POST', path: '/api/v1/catalog/discovery' },
  courseSearch: { method: 'POST', path: '/api/v1/query/courses' },
  sectionSearch: { method: 'POST', path: '/api/v1/query/sections' },
  courseDetail: { method: 'POST', path: '/api/v1/query/course-detail' },
  sectionDetail: { method: 'POST', path: '/api/v1/query/section-detail' },
  openStatus: { method: 'POST', path: '/api/v1/open/status' },
  openSectionStatus: { method: 'POST', path: '/api/v1/open/section-status' },
} as const;

export interface ServiceStatusScope {
  readonly term: string | null;
  readonly campuses: readonly string[];
}

export interface ProductApiPort {
  filterSchema(signal?: AbortSignal): Promise<FilterSchemaV1>;
  filterOptions(
    request: FilterOptionsRequestV2,
    signal?: AbortSignal,
  ): Promise<FilterOptionsResponseV2>;
  validateDynamicFilters(
    request: DynamicFilterValidationRequestV3,
    signal?: AbortSignal,
  ): Promise<DynamicFilterValidationResponseV3>;
  serviceStatus(signal?: AbortSignal, scope?: ServiceStatusScope): Promise<ServiceStatus>;
  catalogDiscovery(
    request: CatalogDiscoveryRequestV1,
    signal?: AbortSignal,
  ): Promise<CatalogDiscoveryResponseV1>;
  searchCourses(
    request: CourseQueryRequestV1,
    signal?: AbortSignal,
  ): Promise<CourseQueryResponseV1>;
  searchSections(
    request: SectionQueryRequestV1,
    signal?: AbortSignal,
  ): Promise<SectionQueryResponseV1>;
  courseDetail(
    request: CourseDetailRequestV1,
    signal?: AbortSignal,
  ): Promise<CourseDetailResponseV1>;
  sectionDetail(
    request: SectionDetailRequestV1,
    signal?: AbortSignal,
  ): Promise<SectionDetailResponseV1>;
  openStatus(
    request: OpenStatusRequestV1,
    signal?: AbortSignal,
  ): Promise<OpenRefreshStatusV1>;
  openSectionStatus(
    request: OpenSectionStatusRequestV1,
    signal?: AbortSignal,
  ): Promise<OpenSectionStatusV1>;
}

export class ProductApi implements ProductApiPort {
  readonly #client: ProductClientPort;

  constructor(client: ProductClientPort) {
    this.#client = client;
  }

  filterSchema(signal?: AbortSignal): Promise<FilterSchemaV1> {
    return this.#client.get<FilterSchemaV1>(PRODUCT_API_ROUTES.filterSchema.path, signal);
  }

  filterOptions(
    request: FilterOptionsRequestV2,
    signal?: AbortSignal,
  ): Promise<FilterOptionsResponseV2> {
    return this.#client.post<FilterOptionsRequestV2, FilterOptionsResponseV2>(
      PRODUCT_API_ROUTES.filterOptions.path,
      request,
      signal,
    );
  }

  validateDynamicFilters(
    request: DynamicFilterValidationRequestV3,
    signal?: AbortSignal,
  ): Promise<DynamicFilterValidationResponseV3> {
    return this.#client.post<DynamicFilterValidationRequestV3, DynamicFilterValidationResponseV3>(
      PRODUCT_API_ROUTES.validateDynamicFilters.path,
      request,
      signal,
    );
  }

  serviceStatus(signal?: AbortSignal, scope?: ServiceStatusScope): Promise<ServiceStatus> {
    const query = new URLSearchParams();
    if (scope?.term !== null && scope?.term !== undefined) query.set('activeTerm', scope.term);
    for (const campus of scope?.campuses ?? []) query.append('activeCampus', campus);
    const suffix = query.size === 0 ? '' : `?${query.toString()}`;
    const path = `${PRODUCT_API_ROUTES.serviceStatus.path}${suffix}`;
    if (scope === undefined) return this.#client.get<ServiceStatus>(path, signal);
    return this.#client.request<never, ServiceStatus>(path, signal === undefined
      ? { authenticate: true, method: 'GET' }
      : { authenticate: true, method: 'GET', signal });
  }

  catalogDiscovery(
    request: CatalogDiscoveryRequestV1,
    signal?: AbortSignal,
  ): Promise<CatalogDiscoveryResponseV1> {
    return this.#client.post<CatalogDiscoveryRequestV1, CatalogDiscoveryResponseV1>(
      PRODUCT_API_ROUTES.catalogDiscovery.path,
      request,
      signal,
    );
  }

  searchCourses(
    request: CourseQueryRequestV1,
    signal?: AbortSignal,
  ): Promise<CourseQueryResponseV1> {
    return this.#client.post<CourseQueryRequestV1, CourseQueryResponseV1>(
      PRODUCT_API_ROUTES.courseSearch.path,
      request,
      signal,
    );
  }

  searchSections(
    request: SectionQueryRequestV1,
    signal?: AbortSignal,
  ): Promise<SectionQueryResponseV1> {
    return this.#client.post<SectionQueryRequestV1, SectionQueryResponseV1>(
      PRODUCT_API_ROUTES.sectionSearch.path,
      request,
      signal,
    );
  }

  courseDetail(
    request: CourseDetailRequestV1,
    signal?: AbortSignal,
  ): Promise<CourseDetailResponseV1> {
    return this.#client.post<CourseDetailRequestV1, CourseDetailResponseV1>(
      PRODUCT_API_ROUTES.courseDetail.path,
      request,
      signal,
    );
  }

  sectionDetail(
    request: SectionDetailRequestV1,
    signal?: AbortSignal,
  ): Promise<SectionDetailResponseV1> {
    return this.#client.post<SectionDetailRequestV1, SectionDetailResponseV1>(
      PRODUCT_API_ROUTES.sectionDetail.path,
      request,
      signal,
    );
  }

  openStatus(
    request: OpenStatusRequestV1,
    signal?: AbortSignal,
  ): Promise<OpenRefreshStatusV1> {
    // The server's batch key is strict (`deny_unknown_fields`), and a Section
    // key satisfies `TermCampusKey` structurally -- `index` included. Only
    // the two batch fields go on the wire, whatever the caller handed over.
    const batch: TermCampusKey = { term: request.batch.term, campus: request.batch.campus };
    return this.#client.post<OpenStatusRequestV1, OpenRefreshStatusV1>(
      PRODUCT_API_ROUTES.openStatus.path,
      { contractVersion: request.contractVersion, batch },
      signal,
    );
  }

  openSectionStatus(
    request: OpenSectionStatusRequestV1,
    signal?: AbortSignal,
  ): Promise<OpenSectionStatusV1> {
    return this.#client.post<OpenSectionStatusRequestV1, OpenSectionStatusV1>(
      PRODUCT_API_ROUTES.openSectionStatus.path,
      request,
      signal,
    );
  }
}
