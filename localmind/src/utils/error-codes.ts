// Error codes from shared-contract/error-code/registry.json
// 6-digit format: [endpoint/module][category][sequence]

export interface ErrorCodeInfo {
  level: 'error' | 'warn' | 'info';
  message: string;
  message_en: string;
}

const errorRegistry: Record<string, ErrorCodeInfo> = {
  '100001': { level: 'error', message: '未知错误', message_en: 'Unknown error' },
  '100002': { level: 'error', message: '内部服务错误', message_en: 'Internal server error' },
  '100003': { level: 'error', message: '网络不可达', message_en: 'Network unreachable' },
  '100004': { level: 'error', message: '请求超时', message_en: 'Request timeout' },
  '100005': { level: 'warn', message: '请求频率过高', message_en: 'Rate limited' },
  '201001': { level: 'error', message: '网关异常', message_en: 'Gateway error' },
  '201002': { level: 'warn', message: '令牌额度已耗尽', message_en: 'Token quota exhausted' },
  '201003': { level: 'warn', message: '离线模型未就绪', message_en: 'Offline model not ready' },
  '201004': { level: 'warn', message: '在线服务暂时不可用', message_en: 'Online service unavailable' },
  '201005': { level: 'info', message: '模型切换中，请稍候', message_en: 'Switching model, please wait' },
  '202001': { level: 'error', message: '上下文过长', message_en: 'Context too long' },
  '202002': { level: 'error', message: '消息发送失败', message_en: 'Message send failed' },
  '301001': { level: 'error', message: '模型下载失败', message_en: 'Model download failed' },
  '301002': { level: 'error', message: '模型校验未通过', message_en: 'Model checksum mismatch' },
  '301003': { level: 'warn', message: '存储空间不足', message_en: 'Insufficient storage' },
  '301004': { level: 'warn', message: '设备不满足运行要求', message_en: 'Device below minimum requirements' },
  '301005': { level: 'error', message: '模型文件损坏', message_en: 'Model file corrupted' },
  '301006': { level: 'error', message: '推理服务启动失败', message_en: 'Inference service start failed' },
  '301007': { level: 'warn', message: '推理服务运行异常', message_en: 'Inference service abnormal' },
  '401001': { level: 'warn', message: '设备未配对', message_en: 'Device not paired' },
  '401002': { level: 'warn', message: '配对码无效或已过期', message_en: 'Pairing code invalid or expired' },
  '401003': { level: 'warn', message: '配对码已使用', message_en: 'Pairing code already used' },
  '401004': { level: 'warn', message: '配对码签发失败', message_en: 'Pairing code generation failed' },
  '402001': { level: 'warn', message: '对端设备离线', message_en: 'Remote device offline' },
  '402002': { level: 'warn', message: '操作不在白名单范围内', message_en: 'Action not in whitelist' },
  '402003': { level: 'warn', message: '危险操作已被拒绝', message_en: 'Dangerous action rejected' },
  '402004': { level: 'warn', message: '远程控制未开启', message_en: 'Remote control disabled' },
  '403001': { level: 'warn', message: '认证票据无效或已过期', message_en: 'Auth ticket invalid or expired' },
  '403002': { level: 'error', message: '越权访问拒绝', message_en: 'Unauthorized access denied' },
  '501001': { level: 'error', message: '配对码签发服务故障', message_en: 'Pairing service error' },
  '501002': { level: 'error', message: '设备注册失败', message_en: 'Device registration failed' },
  '502001': { level: 'error', message: '指令路由失败', message_en: 'Command routing failed' },
  '502002': { level: 'warn', message: '指令执行超时', message_en: 'Command execution timeout' },
  '601001': { level: 'error', message: 'AI 渠道异常', message_en: 'AI channel error' },
  '601002': { level: 'warn', message: '令牌认证失败', message_en: 'Token authentication failed' },
  '602001': { level: 'error', message: '模型映射配置缺失', message_en: 'Model mapping not configured' },
};

export function getErrorInfo(code: string, lang: 'zh' | 'en' = 'zh'): ErrorCodeInfo & { localMessage: string } {
  const info = errorRegistry[code];
  if (!info) {
    return { level: 'error', message: '未知错误', message_en: 'Unknown error', localMessage: '未知错误' };
  }
  return {
    ...info,
    localMessage: lang === 'zh' ? info.message : info.message_en,
  };
}

export function formatErrorMessage(code: string, fallback?: string): string {
  const info = errorRegistry[code];
  if (!info) return fallback || `错误代码: ${code}`;
  return info.message;
}
