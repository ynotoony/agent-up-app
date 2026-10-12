// Input: Hash-based navigation, shared workbench state and local theme preferences.
// Output: Routed workspace shell with project navigation and application notifications.
// Pos: Top-level React application composition.
import { HashRouter, Route, Routes } from 'react-router-dom';
import { Toaster } from 'sonner';
import { ThemeProvider } from '@/components/theme-provider';
import { lazy, Suspense } from 'react';
import { WorkbenchProvider } from '@/lib/workbench';
import { Sidebar } from '@/components/sidebar';
import WorkspacePage, { AttentionPage } from '@/pages/workspace';
import ProjectPage from '@/pages/project';
import ProjectConnectPage from '@/pages/project-connect';
const RequirementPage = lazy(() => import('@/pages/requirement'));
const SettingsPage = lazy(() => import('@/pages/settings'));
const ProjectRecordsPage = lazy(() => import('@/pages/project-records'));

export default function App() {
  return (
    <ThemeProvider>
      <HashRouter>
        <WorkbenchProvider>
        <AppRoutes />
        </WorkbenchProvider>
        <Toaster
          position="top-center"
          toastOptions={{
            style: {
              background: 'var(--popover)',
              color: 'var(--foreground)',
              border: '1px solid var(--border)',
              fontSize: '13px',
            },
          }}
        />
      </HashRouter>
    </ThemeProvider>
  );
}

function AppRoutes() {
  return <>
        <a href="#main-content" className="skip-link" onClick={event => { event.preventDefault(); const main = document.querySelector('main'); if (main) { main.tabIndex = -1; main.focus(); } }}>跳到主要内容</a>
        <div className="app-shell">
          <Sidebar />
          <div className="app-content">
            <Suspense fallback={<div className="board-loading" role="status">正在打开…</div>}>
            <Routes>
              <Route path="/" element={<WorkspacePage />} />
              <Route path="/attention" element={<AttentionPage />} />
              <Route path="/connect" element={<ProjectConnectPage />} />
              <Route path="/project/:id" element={<ProjectPage />} />
              <Route path="/project/:id/records" element={<ProjectRecordsPage />} />
              <Route path="/requirement/:id" element={<RequirementPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="*" element={<WorkspacePage />} />
            </Routes>
            </Suspense>
          </div>
        </div>
  </>;
}
