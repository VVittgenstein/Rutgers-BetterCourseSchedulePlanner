import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import i18n from './i18n';
import './index.css';

const container = document.getElementById('root');
if (!container) {
  throw new Error(i18n.t('errors.runtime.missingRoot'));
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
