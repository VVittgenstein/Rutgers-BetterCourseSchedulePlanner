export interface CourseSearchResponse {
  meta: {
    page: number;
    pageSize: number;
    total: number;
    hasNext: boolean;
    generatedAt: string;
    version: string;
  };
  data: CourseSearchRow[];
}

export interface CourseSearchRow {
  courseId: number;
  termId: string;
  campusCode: string;
  subjectCode: string;
  courseNumber: string;
  courseString: string | null;
  title: string;
  expandedTitle: string | null;
  level: string | null;
  creditsMin: number | null;
  creditsMax: number | null;
  creditsDisplay: string | null;
  coreAttributes: unknown;
  hasOpenSections: boolean;
  sectionsOpen: number;
  updatedAt: string | null;
  prerequisites: string | null;
  subject?: {
    code: string;
    description: string | null;
    schoolCode: string | null;
    schoolDescription: string | null;
  };
  sectionsSummary?: {
    total: number;
    open: number;
    deliveryMethods: string[];
  };
}

export interface FiltersResponse {
  meta: {
    generatedAt: string;
    version: string;
  };
  data: FiltersPayload;
}

export interface FiltersPayload {
  terms: Array<{
    id: string;
    display: string;
    active?: boolean;
  }>;
  campuses: Array<{
    code: string;
    display: string;
    region?: string;
  }>;
  subjects: Array<{
    code: string;
    description: string;
    school?: string;
    campus?: string;
  }>;
  coreCodes: Array<{
    code: string;
    description: string;
  }>;
  levels: string[];
  deliveryMethods: string[];
}
