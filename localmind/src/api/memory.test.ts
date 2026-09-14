import { describe, expect, it } from 'vitest';
import { buildInsightPrompt, parseInsight } from './memory';

describe('parseInsight', () => {
  it('解析标准 JSON', () => {
    const r = parseInsight('{"summary":"做了A","facts":["用户偏好简洁","项目在 X"]}', '原始回复');
    expect(r.summary).toBe('做了A');
    expect(r.facts).toEqual(['用户偏好简洁', '项目在 X']);
  });

  it('容忍 markdown 代码块与前后废话', () => {
    const raw = '好的，结果如下：\n```json\n{"summary":"总结了","facts":[]}\n```\n以上。';
    const r = parseInsight(raw, '原始回复');
    expect(r.summary).toBe('总结了');
    expect(r.facts).toEqual([]);
  });

  it('facts 超过 3 条时截断，并过滤空字符串与非字符串', () => {
    const r = parseInsight('{"summary":"s","facts":["a","b","c","d","",3]}', 'x');
    expect(r.facts).toEqual(['a', 'b', 'c']);
  });

  it('模型没给 JSON 时退化为整段摘要且不猜 facts', () => {
    const r = parseInsight('这轮就是聊了聊天。', '原始回复内容');
    expect(r.summary).toContain('聊了聊天');
    expect(r.facts).toEqual([]);
  });

  it('JSON 里缺 summary 时用回复正文兜底', () => {
    const r = parseInsight('{"facts":["记住这个"]}', '助手回复正文内容');
    expect(r.summary).toContain('助手回复正文');
    expect(r.facts).toEqual(['记住这个']);
  });
});

describe('buildInsightPrompt', () => {
  it('把已有记忆与工具结果带进提示词，并要求 JSON 输出', () => {
    const prompt = buildInsightPrompt(
      '助手回复正文',
      [{ name: 'write_file', output: 'ok', success: true }],
      '# 记忆\n- 用户偏好中文交流\n',
    );
    expect(prompt).toContain('只输出一个 JSON 对象');
    expect(prompt).toContain('用户偏好中文交流');
    expect(prompt).toContain('write_file: OK');
    expect(prompt).toContain('助手回复正文');
  });
});
