import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { PublicCompositionRoot } from './ui/public/PublicCompositionRoot';

const container = document.getElementById('root');

if (container === null) {
  throw new Error('RBCSP public composition root is missing.');
}

createRoot(container).render(
  <StrictMode>
    <PublicCompositionRoot />
  </StrictMode>,
);
