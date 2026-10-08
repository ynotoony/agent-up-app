import { open } from '@tauri-apps/plugin-dialog';
import { toast } from 'sonner';
import type { InitProjectResult } from '../../shared/types';
import { api, errorMessage } from './api';

// 「添加项目」= 初始化目录为项目：系统文件夹选择器（可新建文件夹）→ 后端幂等初始化
// （建目录缺则 + git init + 存量文档扫描入库 + 治理种子 + 登记）。目录零污染，不写任何治理文件。
export async function pickAndCreateProject(): Promise<InitProjectResult | null> {
  let selected: unknown;
  try {
    selected = await open({ directory: true, multiple: false, title: '选择或新建项目文件夹' });
  } catch (err) {
    // 选择器本身失败（权限/插件问题）：必须暴露，不能静默吞掉
    const message = errorMessage(err);
    console.error('[pick-folder]', err);
    toast.error(`无法打开文件夹选择器：${message}`);
    return null;
  }
  if (typeof selected !== 'string' || selected.length === 0) return null; // 用户取消
  try {
    const result = await api.projects.init(selected);
    if (result.already_registered) {
      toast.info(`「${result.project.name}」已是项目，直接打开`);
    } else {
      const r = result.report;
      toast.success(
        `已初始化「${result.project.name}」：文档入库 ${r.docs_added} 篇 · 治理种子 ${r.governance_seeded} 条 · 待定 ${r.pending_open} 条`,
      );
    }
    return result;
  } catch (err) {
    toast.error(errorMessage(err));
    return null;
  }
}
