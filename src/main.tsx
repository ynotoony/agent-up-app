// Input: The HTML root element and bundled application styles.
// Output: A mounted React application.
// Pos: Renderer startup entrypoint.
import { createRoot } from 'react-dom/client';
import App from './App';
import './globals.css';
import './workbench.css';

createRoot(document.getElementById('root')!).render(<App />);
