import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { Toaster } from 'sonner'
import { ProtectedRoute } from '@/components/layout/ProtectedRoute'
import { AppLayout } from '@/components/layout/AppLayout'
import { LoginPage } from '@/pages/LoginPage'
import { DashboardPage } from '@/pages/DashboardPage'
import { FileBrowserPage } from '@/pages/FileBrowserPage'
import { DrivesPage } from '@/pages/DrivesPage'
import { FoldersPage } from '@/pages/FoldersPage'
import { IntegrityPage } from '@/pages/IntegrityPage'
import { SyncQueuePage } from '@/pages/SyncQueuePage'
import { VirtualPathManagerPage } from '@/pages/VirtualPathManagerPage'
import { SchedulerPage } from '@/pages/SchedulerPage'
import { LogsPage } from '@/pages/LogsPage'
import { DatabaseBackupsPage } from '@/pages/DatabaseBackupsPage'

export default function App() {
  return (
    <BrowserRouter>
      <Toaster position="top-right" richColors />
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route
          element={
            <ProtectedRoute>
              <AppLayout />
            </ProtectedRoute>
          }
        >
          <Route index element={<Navigate to="/dashboard" replace />} />
          <Route path="/dashboard" element={<DashboardPage />} />
          <Route path="/files" element={<FileBrowserPage />} />
          <Route path="/drives" element={<DrivesPage />} />
          <Route path="/folders" element={<FoldersPage />} />
          <Route path="/integrity" element={<IntegrityPage />} />
          <Route path="/sync" element={<SyncQueuePage />} />
          <Route path="/virtual-paths" element={<VirtualPathManagerPage />} />
          <Route path="/scheduler" element={<SchedulerPage />} />
          <Route path="/logs" element={<LogsPage />} />
          <Route path="/database" element={<DatabaseBackupsPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </BrowserRouter>
  )
}
