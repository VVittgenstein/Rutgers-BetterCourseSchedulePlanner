import type { FiltersDictionary } from '../components/FilterPanel';
import type { CourseFilterState, DeliveryMethod } from '../state/courseFilters';
import { apiGet } from './client';
import type { FiltersResponse } from './types';

const DELIVERY_LABELS: Record<DeliveryMethod, string> = {
  in_person: 'In Person',
  online: 'Online',
  hybrid: 'Hybrid',
};

const LEVEL_LABELS: Record<string, string> = {
  UG: 'Undergraduate',
  GR: 'Graduate',
  'N/A': 'Other',
};

export async function fetchFiltersDictionary(signal?: AbortSignal): Promise<FiltersDictionary> {
  const payload = await apiGet<FiltersResponse>('/filters', undefined, signal);
  const data = payload.data ?? {
    terms: [],
    campuses: [],
    subjects: [],
    coreCodes: [],
    levels: [],
    deliveryMethods: [],
  };

  return {
    terms: data.terms.map((term) => ({
      label: term.display ?? term.id,
      value: term.id,
      description: term.active ? 'Active' : undefined,
    })),
    campuses: data.campuses.map((campus) => ({
      label: campus.display ?? campus.code,
      value: campus.code,
      description: campus.region,
    })),
    subjects: data.subjects.map((subject) => ({
      label: subject.description ?? subject.code,
      value: subject.code,
      school: subject.school ?? subject.campus ?? undefined,
    })),
    levels: normalizeLevels(data.levels),
    deliveries: normalizeDeliveryMethods(data.deliveryMethods),
    tags: [],
    coreCodes: data.coreCodes.map((core) => ({
      label: core.description ?? core.code,
      value: core.code,
    })),
    instructors: [],
  };
}

function normalizeLevels(levels: string[]) {
  const result: FiltersDictionary['levels'] = [];
  const seen = new Set<string>();
  for (const level of levels) {
    if (seen.has(level)) continue;
    if (LEVEL_LABELS[level]) {
      result.push({
        label: LEVEL_LABELS[level],
        value: level as CourseFilterState['level'][number],
      });
      seen.add(level);
    }
  }

  if (!result.length) {
    return [
      { label: LEVEL_LABELS.UG, value: 'UG' as CourseFilterState['level'][number] },
      { label: LEVEL_LABELS.GR, value: 'GR' as CourseFilterState['level'][number] },
    ];
  }

  return result;
}

function normalizeDeliveryMethods(methods: string[]) {
  const result: FiltersDictionary['deliveries'] = [];
  const seen = new Set<string>();
  for (const method of methods) {
    if (seen.has(method)) continue;
    if (DELIVERY_LABELS[method as DeliveryMethod]) {
      result.push({
        label: DELIVERY_LABELS[method as DeliveryMethod],
        value: method as DeliveryMethod,
      });
      seen.add(method);
    }
  }

  if (!result.length) {
    return [
      { label: DELIVERY_LABELS.in_person, value: 'in_person' as DeliveryMethod },
      { label: DELIVERY_LABELS.online, value: 'online' as DeliveryMethod },
      { label: DELIVERY_LABELS.hybrid, value: 'hybrid' as DeliveryMethod },
    ];
  }

  return result;
}
