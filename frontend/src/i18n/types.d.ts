import 'i18next';

import messages from '../../i18n/messages.json';

declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    resources: {
      translation: (typeof messages)['en'];
    };
  }
}
