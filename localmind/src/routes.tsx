import type { RouteObject } from 'react-router-dom';
import ChatPage from './pages/ChatPage';
import ModelsPage from './pages/ModelsPage';
import SettingsPage from './pages/SettingsPage';

export const routes: RouteObject[] = [
  { path: '/', element: <ChatPage /> },
  { path: '/models', element: <ModelsPage /> },
  { path: '/settings', element: <SettingsPage /> },
];