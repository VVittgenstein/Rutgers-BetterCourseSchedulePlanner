import type { FiltersDictionary } from '../components/FilterPanel';

export const fallbackFiltersDictionary: FiltersDictionary = {
  terms: [
    { label: 'Fall 2024', value: '2024FA' },
    { label: 'Spring 2025', value: '2025SP' },
  ],
  campuses: [
    { label: 'New Brunswick', value: 'NB' },
    { label: 'Newark', value: 'NWK' },
    { label: 'Camden', value: 'CAM' },
  ],
  subjects: [
    { label: 'Computer Science', value: '01:198', school: 'SAS' },
    { label: 'Mathematics', value: '01:640', school: 'SAS' },
    { label: 'Statistics', value: '01:960', school: 'SAS' },
    { label: 'Electrical Engineering', value: '14:332', school: 'SOE' },
    { label: 'Psychology', value: '01:830', school: 'SAS' },
    { label: 'Philosophy', value: '01:730', school: 'SAS' },
  ],
  levels: [
    { label: 'Undergraduate', value: 'UG' },
    { label: 'Graduate', value: 'GR' },
  ],
  deliveries: [
    { label: 'In Person', value: 'in_person' },
    { label: 'Online', value: 'online' },
    { label: 'Hybrid', value: 'hybrid' },
  ],
  tags: [
    { label: 'Writing Intensive', value: 'writing' },
    { label: 'Requires Permission', value: 'permission' },
    { label: 'STEM', value: 'stem' },
  ],
  coreCodes: [
    { label: 'Q: Quantitative', value: 'QQ' },
    { label: 'W: Writing', value: 'W' },
  ],
  instructors: [
    { label: 'Prof. Rivera', value: 'rivera' },
    { label: 'Prof. Nguyen', value: 'nguyen' },
  ],
};
