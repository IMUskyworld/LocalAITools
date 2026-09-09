// 电脑操作工具定义与执行器
// 定义 OpenAI 兼容的工具清单（供 AI 调用），并提供各工具的 IPC 执行函数
// 注意：工具数量经过验证——本地小模型（qwen2.5:7b）在工具过多时不稳定，
// 精简为 3 个核心工具保证可靠调用

import { tauriInvoke } from '@/api/ipc';

// ========== OpenAI 兼容工具定义（JSON Schema） ==========

export interface ToolDef {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: {
      type: 'object';
      properties: Record<string, any>;
      required?: string[];
    };
  };
}

/**
 * 构建工具定义清单。桌面路径动态注入，避免硬编码到某台电脑。
 * 工具数量经过验证——本地小模型（qwen2.5:7b）在工具过多时不稳定，
 * 精简为 3 个核心工具保证可靠调用。
 */
export function buildToolDefs(desktopPath: string = ''): ToolDef[] {
  const pathExample = desktopPath ? `${desktopPath}/test.py` : '文件的完整路径';
  return [
  {
    type: 'function',
    function: {
      name: 'write_file',
      description: '创建新文件或覆盖写一个文本文件。目录不存在会自动创建。用于生成代码、脚本、文档、配置文件、笔记等。',
      parameters: {
        type: 'object',
        properties: {
          path: { type: 'string', description: `文件的完整路径，如 ${pathExample}` },
          content: { type: 'string', description: '要写入的完整文件内容' },
        },
        required: ['path', 'content'],
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'read_file',
      description: '读取文本文件内容。用于查看代码、配置、日志、笔记等文件内容。',
      parameters: {
        type: 'object',
        properties: {
          path: { type: 'string', description: '文件的完整路径' },
        },
        required: ['path'],
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'create_doc',
      description: '生成文档文件（PPT/Word/Excel/PDF）。用户要求制作演示文稿、文档、表格、PDF 时使用。',
      parameters: {
        type: 'object',
        properties: {
          doc_type: {
            type: 'string',
            enum: ['ppt', 'docx', 'xlsx', 'pdf'],
            description: '文档类型：ppt=演示文稿, docx=Word文档, xlsx=Excel表格, pdf=PDF',
          },
          spec: {
            type: 'object',
            description: '文档内容规格，必须包含 path 字段指定输出路径。按 doc_type 不同结构：ppt 含 title/slides；docx 含 title/paragraphs；xlsx 含 sheet_name/headers/rows；pdf 含 title/sections',
          },
        },
        required: ['doc_type', 'spec'],
      },
    },
  },
];
}

// ========== 工具执行器 ==========

export interface ToolResult {
  success: boolean;
  output: string;
  error: string;
}

/**
 * 根据工具名执行对应 IPC 命令
 */
export async function executeTool(name: string, args: Record<string, any>): Promise<ToolResult> {
  try {
    const result = await tauriInvoke(name, args);
    if (result?.data) {
      return result.data as ToolResult;
    }
    return {
      success: false,
      output: '',
      error: result?.error || `工具 ${name} 返回异常`,
    };
  } catch (e: any) {
    return {
      success: false,
      output: '',
      error: `${name} 执行失败: ${e?.message || e || '未知错误'}`,
    };
  }
}

/**
 * 获取常见系统路径（供 AI 了解用户电脑结构）
 */
export async function getCommonPaths(): Promise<string> {
  try {
    const result = await tauriInvoke('get_common_paths');
    if (result?.data) return (result.data as string[]).join('\n');
  } catch {}
  return '';
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
 * 获取真实桌面路径（供 system prompt 和 tool defs 注入）。
 * 拿不到时返回空串，调用方做兜底。
 */
export async function getDesktopPath(): Promise<string> {
  const pathsText = await getCommonPaths();
  return parseDesktopPath(pathsText);
}
