// 系统路径查询辅助。
// Phase 0 已删除无调用点的工具定义与 JS 工具执行器；Agent 工具统一由 Python Registry 管理。

import { tauriInvoke } from '@/api/ipc';

/**
 * 获取常见系统路径（供 AI 了解用户电脑结构）
 */
export async function getCommonPaths(): Promise<string> {
  const result = await tauriInvoke<string[]>('get_common_paths');
  return Array.isArray(result.data) ? result.data.join('\n') : '';
}

/**
 * 解析 getCommonPaths 返回文本中的桌面路径。
 * Rust 端 get_common_paths 返回 "桌面: C:/Users/xxx/OneDrive/Desktop" 格式。
 */
export function parseDesktopPath(pathsText: string): string {
  if (!pathsText) return '';
  const line = pathsText.split('\n').find((l) => l.startsWith('桌面:'));
  if (!line) return '';
  const desktop = line.replace('桌面:', '').trim();
  return desktop || '';
}

/**
 * 获取真实桌面路径（供 system prompt 和工具注入）。
 * 拿不到时返回空串，调用方做兜底。
 */
export async function getDesktopPath(): Promise<string> {
  const pathsText = await getCommonPaths();
  return parseDesktopPath(pathsText);
}
