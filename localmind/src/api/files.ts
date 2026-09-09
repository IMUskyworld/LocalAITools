// 文件处理 API 封装
// 前端选文件 → 调 Rust IPC 读取内容

import { tauriInvoke } from '@/api/ipc';
import { open } from '@tauri-apps/plugin-dialog';

export interface FileReadResult {
  path: string;
  file_name: string;
  extension: string;
  size_bytes: number;
  content: string;
  truncated: boolean;
}

export interface SelectedAttachment {
  id: string;
  path: string;
  file_name: string;
  extension: string;
  size_bytes: number;
  content: string;
  truncated: boolean;
}

let seq = 0;
function genId() {
  seq += 1;
  return `attach-${Date.now()}-${seq}`;
}

/**
 * 弹出文件选择框，读取选中的文件内容
 * 返回 null 表示用户取消
 */
export async function pickAndReadFile(): Promise<SelectedAttachment | null> {
  try {
    const path = await open({
      multiple: false,
      directory: false,
      title: '选择要处理的文件',
    });
    if (typeof path !== 'string' || !path) return null;

    const result = await tauriInvoke('read_text_file', { path });
    const data: FileReadResult | undefined = result?.data;
    if (!data) {
      throw new Error(result?.error || '读取文件失败');
    }
    return {
      id: genId(),
      path: data.path,
      file_name: data.file_name,
      extension: data.extension,
      size_bytes: data.size_bytes,
      content: data.content,
      truncated: data.truncated,
    };
  } catch (e: any) {
    console.warn('[files] pickAndReadFile failed:', e);
    throw e;
  }
}

/**
 * 格式化文件大小
 */
export function formatFileSize(bytes: number): string {
  if (!bytes) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
