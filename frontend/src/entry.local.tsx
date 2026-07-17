import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { LocalCompositionRoot } from './ui/local/LocalCompositionRoot';

const container = document.getElementById('root');

if (container === null) {
  throw new Error('RBCSP local composition root is missing.');
}

createRoot(container).render(
  <StrictMode>
    <LocalCompositionRoot />
  </StrictMode>,
);
