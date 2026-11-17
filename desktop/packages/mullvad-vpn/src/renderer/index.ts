import { createRoot } from 'react-dom/client';

import log from '../shared/logging';
import App from './app';
import { installRendererErrorHandlers } from './lib/renderer-error-handlers';

installRendererErrorHandlers(window, log);

const app = new App();
const container = document.getElementById('app');
const root = createRoot(container!);
root.render(app.renderView());
