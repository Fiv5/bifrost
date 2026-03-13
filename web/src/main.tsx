import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';
import { StartupSplashHost } from './components/StartupSplash';
import { isDesktopShell } from './runtime';

const rootElement = document.getElementById('root');
const startupSplashElement = document.getElementById('startup-splash');

if (!rootElement) {
  throw new Error('Missing root element');
}

if (isDesktopShell() && startupSplashElement) {
  ReactDOM.hydrateRoot(
    startupSplashElement,
    <React.StrictMode>
      <StartupSplashHost />
    </React.StrictMode>,
  );
} else {
  startupSplashElement?.remove();
}

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
