import { HashRouter, Route, Routes } from 'react-router-dom';
import { Toaster } from 'sonner';
import { ThemeProvider } from '@/components/theme-provider';
import { Sidebar } from '@/components/sidebar';
import WorkspacePage from '@/pages/workspace';
import ProjectPage from '@/pages/project';
import RequirementPage from '@/pages/requirement';
import SettingsPage from '@/pages/settings';

export default function App() {
  return (
    <ThemeProvider>
      <HashRouter>
        <div className="flex h-screen">
          <Sidebar />
          <div className="min-w-0 flex-1">
            <Routes>
              <Route path="/" element={<WorkspacePage />} />
              <Route path="/project/:id" element={<ProjectPage />} />
              <Route path="/requirement/:id" element={<RequirementPage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Routes>
          </div>
        </div>
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
