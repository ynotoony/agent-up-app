import { useEffect, useRef, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// 流式事件订阅：requirement://{id}/stream。
// 事件形状与后端 emit_stream 的 payload 一致：
// { task_id, stage, event: { kind: 'delta'|'message'|'notice', text } }

export interface StreamEventPayload {
  task_id: string;
  stage: string;
  event: { kind: 'delta' | 'message' | 'notice'; text: string };
}

export interface StreamState {
  /** 是否有活跃流（收到过事件且未完成） */
  active: boolean;
  /** 当前流式阶段（understand/plan/implement/verify/review） */
  stage: string | null;
  /** 增量文本累计（delta 逐段追加） */
  streamText: string;
  /** 最近一条消息级完整文本（message 事件覆盖） */
  lastMessage: string;
  /** 事件是否仍在推进（用于光标动画） */
  ticking: boolean;
}

const IDLE_AFTER_MS = 4000;

const EMPTY_STREAM_STATE: StreamState = {
  active: false,
  stage: null,
  streamText: '',
  lastMessage: '',
  ticking: false,
};

const EMPTY_ACCUMULATOR = {
  streamText: '',
  lastMessage: '',
  stage: null as string | null,
  active: false,
};

/**
 * 订阅某条需求的流式事件。
 * - delta 事件：追加 streamText（token 级）
 * - message 事件：覆盖 lastMessage，并重置 streamText（新一轮完整消息）
 * - 前端离开页面自动退订；4s 无新事件视为流停滞（隐藏光标）
 */
export function useRequirementStream(requirementId: string | undefined): StreamState {
  const [state, setState] = useState<StreamState>(EMPTY_STREAM_STATE);
  // 用 ref 存累计文本避免高频 setState 闭包竞态；渲染节流到 rAF
  const accRef = useRef({ streamText: '', lastMessage: '', stage: null as string | null, active: false });
  const lastEventAtRef = useRef(0);
  const rafRef = useRef<number | null>(null);
  const idleTimerRef = useRef<number | null>(null);
  // Keep this id at the last effect boundary. During the render immediately
  // after navigation it still points at the previous subscription, so the
  // return value can hide stale output until the reset effect runs.
  const subscribedIdRef = useRef<string | undefined>(undefined);
  const idChanged = subscribedIdRef.current !== requirementId;

  useEffect(() => {
    // A component instance can be reused while navigating between requirements.
    // Clear the accumulator before subscribing so output from the previous id
    // can never be rendered for the next one.
    accRef.current = { ...EMPTY_ACCUMULATOR };
    lastEventAtRef.current = 0;
    subscribedIdRef.current = requirementId;
    setState(EMPTY_STREAM_STATE);
    if (!requirementId) return;
    let unlisten: UnlistenFn | null = null;
    let disposed = false;

    const flush = () => {
      rafRef.current = null;
      setState({
        active: accRef.current.active,
        stage: accRef.current.stage,
        streamText: accRef.current.streamText,
        lastMessage: accRef.current.lastMessage,
        ticking: Date.now() - lastEventAtRef.current < IDLE_AFTER_MS,
      });
    };
    const scheduleFlush = () => {
      if (rafRef.current === null) {
        rafRef.current = requestAnimationFrame(flush);
      }
    };
    const markIdle = () => {
      if (idleTimerRef.current !== null) window.clearTimeout(idleTimerRef.current);
      idleTimerRef.current = window.setTimeout(() => {
        lastEventAtRef.current = 0;
        if (accRef.current.active) {
          accRef.current = { ...accRef.current, active: false };
          flush();
        }
      }, IDLE_AFTER_MS);
    };

    void listen<StreamEventPayload>(`requirement://${requirementId}/stream`, (e) => {
      const { stage, event } = e.payload;
      lastEventAtRef.current = Date.now();
      if (!accRef.current.active || accRef.current.stage !== stage) {
        // 新阶段开始：清空上一阶段文本
        accRef.current = { streamText: '', lastMessage: '', stage, active: true };
      }
      if (event.kind === 'delta') {
        accRef.current.streamText += event.text;
      } else if (event.kind === 'message') {
        accRef.current.lastMessage = event.text;
        accRef.current.streamText = '';
      }
      scheduleFlush();
      markIdle();
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    }).catch(() => {
      // Subscription failure must not leave stale output marked as active.
      if (!disposed) {
        accRef.current = { ...EMPTY_ACCUMULATOR };
        lastEventAtRef.current = 0;
        setState(EMPTY_STREAM_STATE);
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
      if (idleTimerRef.current !== null) window.clearTimeout(idleTimerRef.current);
    };
  }, [requirementId]);

  // Effects run after the first render following a route-param change. Return
  // an empty value for that render too, so the old stream cannot flash in the
  // newly selected requirement.
  return idChanged ? EMPTY_STREAM_STATE : state;
}
