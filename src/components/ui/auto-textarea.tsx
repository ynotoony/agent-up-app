import { forwardRef, useCallback, useLayoutEffect, useRef, type TextareaHTMLAttributes } from 'react';

export interface AutoTextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  maxAutoHeight?: number;
}

export const AutoTextarea = forwardRef<HTMLTextAreaElement, AutoTextareaProps>(function AutoTextarea(
  { maxAutoHeight = 360, className = '', onInput, value, defaultValue, ...props },
  ref,
) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const setTextareaRef = useCallback((node: HTMLTextAreaElement | null) => {
    textareaRef.current = node;
    if (typeof ref === 'function') ref(node);
    else if (ref) ref.current = node;
  }, [ref]);

  const resize = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = 'auto';
    const nextHeight = Math.min(textarea.scrollHeight, maxAutoHeight);
    textarea.style.height = `${Math.max(nextHeight, 0)}px`;
    textarea.style.overflowY = textarea.scrollHeight > maxAutoHeight ? 'auto' : 'hidden';
  }, [maxAutoHeight]);

  useLayoutEffect(() => {
    resize();
  }, [resize, value, defaultValue]);

  return (
    <textarea
      {...props}
      ref={setTextareaRef}
      value={value}
      defaultValue={defaultValue}
      onInput={(event) => {
        resize();
        onInput?.(event);
      }}
      className={`resize-none overflow-y-hidden ${className}`}
    />
  );
});
