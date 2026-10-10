import { useLayoutEffect, useState } from 'react';
import { Cpu, FileCog, Loader2, RotateCcw, Save, X } from 'lucide-react';
import type { OrchestrationSettings, RoleInfo } from '../../shared/types';
import { api, errorMessage, type RuntimeInfo } from '@/lib/api';
import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Select, SelectChevron } from '@/components/ui/select';
import { AutoTextarea } from '@/components/ui/auto-textarea';
import { PanelHeader } from '@/components/understanding-panel';
import { toast } from 'sonner';

// 设置页：执行编排 —— 阶段 × 运行时 × 角色 agentmd。
// 默认运行时在本机运行时列表行内设置；每个 AI 阶段一行：选运行时（未指定跟随默认）、编辑角色文件（userData/roles/*.agent.md）。

const STAGES: { key: string; label: string; readonly?: boolean }[] = [
  { key: 'understand', label: '需求理解', readonly: true },
  { key: 'plan', label: '方案规划', readonly: true },
  { key: 'implement', label: '代码实施' },
  { key: 'verify', label: '验证审查', readonly: true },
  { key: 'review', label: '交付审查', readonly: true },
];

export default function SettingsPage() {
  const [runtimes, setRuntimes] = useState<RuntimeInfo[]>([]);
  const [orch, setOrch] = useState<OrchestrationSettings>({ default_runtime: null, stage_runtimes: {} });
  const [roles, setRoles] = useState<RoleInfo[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchData = () => {
    Promise.all([api.runtimes.list(), api.orchestration.get(), api.roles.list()])
      .then(([runtimeList, orchSettings, roleList]) => {
        setRuntimes(runtimeList);
        setOrch(orchSettings);
        setRoles(roleList);
      })
      .catch((err) => toast.error(errorMessage(err)))
      .finally(() => setLoading(false));
  };

  useLayoutEffect(fetchData, []);

  const saveOrch = async (next: OrchestrationSettings, successMsg = '执行编排已保存') => {
    setOrch(next);
    try {
      const saved = await api.orchestration.save({
        default_runtime: next.default_runtime,
        stage_runtimes: next.stage_runtimes,
      });
      setOrch(saved);
      toast.success(successMsg);
    } catch (err) {
      toast.error(errorMessage(err));
    }
  };

  if (loading) {
    return (
      <main className="h-screen overflow-y-auto scrollbar-thin">
        <div className="max-w-4xl mx-auto px-6 py-8 space-y-4 animate-pulse">
          <div className="h-8 w-32 rounded bg-muted" />
          <div className="h-40 rounded-xl bg-card border border-border" />
          <div className="h-64 rounded-xl bg-card border border-border" />
        </div>
      </main>
    );
  }

  const availableCount = runtimes.filter((r) => r.available).length;
  const defaultRuntimeName = orch.default_runtime
    ? runtimes.find((r) => r.id === orch.default_runtime)?.name ?? orch.default_runtime
    : '注册表默认（Codex CLI）';

  return (
    <main className="h-screen overflow-y-auto scrollbar-thin">
      <div className="max-w-4xl mx-auto px-6 py-8 space-y-6">
        <header>
          <h1 className="text-lg font-semibold tracking-tight">设置</h1>
          <p className="mt-0.5 text-xs text-muted-foreground">本机运行时、阶段编排与角色指令</p>
        </header>

        <section className="bg-card border border-border rounded-xl overflow-hidden">
          <PanelHeader
            title="本机运行时"
            icon={<Cpu className="w-3.5 h-3.5" />}
            right={<span className="text-[11px] text-muted-foreground/60">{availableCount > 0 ? `${availableCount} 个可用` : '未探测到可用 CLI'}</span>}
          />
          <div className="divide-y divide-border">
            {runtimes.map((runtime) => (
              <RuntimeRow
                key={runtime.id}
                runtime={runtime}
                isDefault={orch.default_runtime === runtime.id}
                onSetDefault={() => void saveOrch({ ...orch, default_runtime: runtime.id }, '已设为默认运行时')}
              />
            ))}
          </div>
          <div className="px-5 py-2.5 border-t border-border bg-muted/30">
            <p className="text-[11px] text-muted-foreground/60 leading-relaxed">
              未找到 CLI？安装后重启应用即可探测到；也可用环境变量 AGENTUP_CODEX / AGENTUP_OPENCODE 指定可执行文件路径。
            </p>
          </div>
        </section>

        <section className="bg-card border border-border rounded-xl overflow-hidden">
          <PanelHeader
            title="执行编排"
            icon={<FileCog className="w-3.5 h-3.5" />}
          />
          <div className="divide-y divide-border">
            {STAGES.map((stage) => (
              <StageRow
                key={stage.key}
                stage={stage}
                runtimes={runtimes}
                orch={orch}
                role={roles.find((r) => r.stage === stage.key)}
                onSaveRuntime={(value) =>
                  void saveOrch({
                    ...orch,
                    stage_runtimes: { ...orch.stage_runtimes, [stage.key]: value ?? undefined },
                  })
                }
                onRoleChanged={(role) => setRoles((prev) => prev.map((r) => (r.stage === role.stage ? role : r)))}
              />
            ))}
          </div>
          <div className="px-5 py-3 border-t border-border bg-muted/30 flex items-center gap-3 flex-wrap">
            <span className="text-xs text-muted-foreground">未指定的阶段使用默认运行时</span>
            <span className="text-xs font-medium text-foreground">{defaultRuntimeName}</span>
            <span className="text-[11px] text-muted-foreground/50">在上方「本机运行时」列表设置 · 验证/审查阶段自动加只读沙箱（若运行时支持）</span>
          </div>
        </section>
      </div>
    </main>
  );
}

function RuntimeRow({
  runtime,
  isDefault,
  onSetDefault,
}: {
  runtime: RuntimeInfo;
  isDefault: boolean;
  onSetDefault: () => void;
}) {
  const [checking, setChecking] = useState(false);
  const [checkResult, setCheckResult] = useState<{ ok: boolean; text: string } | null>(null);

  const runCheck = async () => {
    setChecking(true);
    setCheckResult(null);
    try {
      const result = await api.runtimes.check(runtime.id);
      setCheckResult(
        result.ok
          ? { ok: true, text: `连通正常 · ${(result.latency_ms! / 1000).toFixed(1)}s · 回复「${result.reply}」` }
          : { ok: false, text: result.error ?? '检测失败' },
      );
    } catch (err) {
      setCheckResult({ ok: false, text: errorMessage(err) });
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="flex items-center gap-3 px-5 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-sm font-medium text-foreground">{runtime.name}</span>
          <span
            className={cn(
              'px-1.5 py-0.5 rounded text-[11px]',
              runtime.available
                ? 'bg-emerald-600/10 dark:bg-emerald-400/10 text-emerald-600 dark:text-emerald-400'
                : 'bg-muted text-muted-foreground/60'
            )}
          >
            {runtime.available ? '可用' : '未找到'}
          </span>
          {isDefault && (
            <span
              className={cn(
                'px-1.5 py-0.5 rounded text-[11px]',
                runtime.available
                  ? 'bg-primary/10 text-primary'
                  : 'bg-amber-600/10 dark:bg-amber-400/10 text-amber-600 dark:text-amber-400'
              )}
            >
              默认{!runtime.available && ' · 未找到，执行时自动回退'}
            </span>
          )}
          {runtime.read_only_analysis ? (
            <span className="px-1.5 py-0.5 rounded text-[11px] bg-muted text-muted-foreground">可强制只读</span>
          ) : (
            <span className="px-1.5 py-0.5 rounded text-[11px] bg-muted text-muted-foreground">无沙箱</span>
          )}
        </div>
        <p className="mt-0.5 text-xs text-muted-foreground/70 line-clamp-1">{runtime.description}</p>
        {runtime.path && (
          <p className="mt-0.5 text-[11px] text-muted-foreground/50 font-mono truncate">
            {runtime.path}
            {runtime.version ? ` · ${runtime.version}` : ''}
            {runtime.model
              ? ` · 模型 ${runtime.model}`
              : runtime.id === 'zcode-app'
                ? ' · 模型跟随桌面 App 登录凭据'
                : ' · 模型未配置（用 CLI 默认值）'}
          </p>
        )}
        {checkResult && (
          <p
            className={cn(
              'mt-0.5 text-[11px] flex items-center gap-1',
              checkResult.ok ? 'text-emerald-600 dark:text-emerald-400' : 'text-destructive'
            )}
          >
            {checkResult.ok ? '✓' : '✗'}
            <span className="truncate">{checkResult.text}</span>
          </p>
        )}
      </div>
      <div className="shrink-0 flex items-center gap-1.5">
        {runtime.available && (
          <Button size="sm" variant="secondary" onClick={() => void runCheck()} loading={checking} title="发一个最小 prompt 验证 CLI/API/模型全链路（最长 60s）">
            {checking ? '检测中…' : checkResult ? (checkResult.ok ? '✓ 重新检测' : '✗ 重新检测') : '连通性检测'}
          </Button>
        )}
        {runtime.available && !isDefault && <Button size="sm" variant="secondary" onClick={onSetDefault}>设为默认</Button>}
      </div>
    </div>
  );
}

function StageRow({
  stage,
  runtimes,
  orch,
  role,
  onSaveRuntime,
  onRoleChanged,
}: {
  stage: { key: string; label: string; readonly?: boolean };
  runtimes: RuntimeInfo[];
  orch: OrchestrationSettings;
  role?: RoleInfo;
  onSaveRuntime: (value: string | null) => void;
  onRoleChanged: (role: RoleInfo) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const configured = orch.stage_runtimes[stage.key];
  const resolved = configured ?? orch.default_runtime ?? 'codex-cli';
  const runtime = runtimes.find((r) => r.id === resolved);
  const custom = Boolean(configured);
  const edited = role?.modified_at != null;

  const openEditor = () => {
    setDraft(role?.content ?? '');
    setEditing(true);
  };

  const save = async () => {
    if (!role) return;
    setBusy(true);
    try {
      const saved = await api.roles.save(role.stage, draft);
      onRoleChanged(saved);
      setEditing(false);
      toast.success(`${role.label}角色已保存`);
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const reset = async () => {
    if (!role) return;
    setBusy(true);
    try {
      const saved = await api.roles.reset(role.stage);
      onRoleChanged(saved);
      setDraft(saved.content);
      toast.success(`${role.label}角色已恢复默认`);
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="px-5 py-3.5">
      <div className="flex items-center gap-3">
        <div className="w-24 shrink-0 flex items-center gap-1.5">
          <span className="text-sm font-medium text-foreground">{stage.label}</span>
        </div>
        <div className="flex-1 min-w-0 flex items-center gap-2 flex-wrap">
          <span className="relative inline-block">
            <Select
              className="h-8 text-xs pr-7 w-44"
              value={configured ?? ''}
              onChange={(e) => onSaveRuntime(e.target.value || null)}
            >
              <option value="">跟随默认{runtime && !custom ? `（${runtime.name}）` : ''}</option>
              {runtimes.filter((r) => r.available).map((r) => (
                <option key={r.id} value={r.id}>
                  {r.name}
                </option>
              ))}
            </Select>
            <SelectChevron />
          </span>
          {stage.readonly && (
            <Badge pill={false} className="text-[11px] text-muted-foreground bg-muted">
              只读沙箱
            </Badge>
          )}
          {edited && (
            <span className="text-[11px] text-muted-foreground/50">角色已自定义</span>
          )}
        </div>
        <Button size="sm" variant="secondary" onClick={openEditor}>
          <FileCog className="w-3.5 h-3.5" />
          角色
        </Button>
      </div>

      {editing && role && (
        <div className="mt-3 space-y-2">
          <AutoTextarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            rows={10}
            maxAutoHeight={520}
            spellCheck={false}
            className="w-full bg-background border border-border rounded-lg px-3 py-2 text-xs font-mono leading-relaxed focus:outline-none focus:border-primary/50 focus:ring-1 focus:ring-primary/20"
          />
          <p className="text-[11px] text-muted-foreground/50 leading-relaxed">
            角色只管「怎么想」；输出 JSON 契约由引擎强制追加，删掉也会自动补回。
            文件位置：<span className="font-mono">{role.path}</span>
          </p>
          <div className="flex items-center gap-2">
            <Button size="sm" onClick={() => void save()} loading={busy} disabled={draft === role.content}>
              <Save className="w-3.5 h-3.5" />
              保存
            </Button>
            <Button size="sm" variant="secondary" onClick={() => void reset()} loading={busy}>
              <RotateCcw className="w-3.5 h-3.5" />
              恢复默认
            </Button>
            <Button size="sm" variant="secondary" onClick={() => setEditing(false)}>
              <X className="w-3.5 h-3.5" />
              取消
            </Button>
            {busy && <Loader2 className="w-3.5 h-3.5 animate-spin text-muted-foreground" />}
          </div>
        </div>
      )}
    </div>
  );
}
