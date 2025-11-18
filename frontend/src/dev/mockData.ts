import type { FiltersDictionary } from '../components/FilterPanel';
import type { ScheduleSection } from '../components/SchedulePreview';

export const mockDictionary: FiltersDictionary = {
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

export const mockSections: ScheduleSection[] = [
  {
    id: 'cs-111-01',
    title: 'Introduction to Computer Science',
    courseCode: '01:198:111',
    sectionCode: '01',
    instructor: 'Prof. Rivera',
    location: 'Hill 120',
    color: '#7c3aed',
    meetings: [
      { day: 'M', startMinutes: 9 * 60, endMinutes: 10 * 60 + 20 },
      { day: 'W', startMinutes: 9 * 60, endMinutes: 10 * 60 + 20 },
    ],
  },
  {
    id: 'math-151-06',
    title: 'Calculus I',
    courseCode: '01:640:151',
    sectionCode: '06',
    instructor: 'Prof. Liu',
    location: 'ARC 103',
    color: '#0ea5e9',
    meetings: [
      { day: 'T', startMinutes: 8 * 60 + 40, endMinutes: 10 * 60 },
      { day: 'F', startMinutes: 8 * 60 + 40, endMinutes: 10 * 60 },
    ],
  },
  {
    id: 'psych-101-05',
    title: 'General Psychology',
    courseCode: '01:830:101',
    sectionCode: '05',
    instructor: 'Prof. Allen',
    location: 'Tillett 226',
    color: '#f97316',
    meetings: [
      { day: 'M', startMinutes: 12 * 60 + 10, endMinutes: 13 * 60 + 30 },
      { day: 'TH', startMinutes: 12 * 60 + 10, endMinutes: 13 * 60 + 30 },
    ],
  },
  {
    id: 'cs-213-02',
    title: 'Systems Programming',
    courseCode: '01:198:213',
    sectionCode: '02',
    instructor: 'Prof. Patel',
    location: 'CORE 101',
    color: '#10b981',
    meetings: [
      { day: 'T', startMinutes: 14 * 60 + 30, endMinutes: 15 * 60 + 50 },
      { day: 'F', startMinutes: 14 * 60 + 30, endMinutes: 15 * 60 + 50 },
    ],
  },
];
