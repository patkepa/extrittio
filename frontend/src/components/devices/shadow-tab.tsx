import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Callout, Divider, Spinner, Tag } from '@blueprintjs/core';
import { useDeviceShadow, useUpdateDesiredState, useDeleteDeviceShadow } from '../../hooks/use-shadow';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';

/**
 * Classify each line of the edited text vs the original text.
 * Uses a simple Set-based approach: lines present only in the edit are "added",
 * lines present only in the original are implicitly gone (shown via changed lines).
 * For the overlay we just need per-line coloring of the *edited* text.
 */
function classifyLines(
  originalText: string,
  editedText: string
): ('unchanged' | 'added' | 'modified')[] {
  const origLines = originalText.split('\n');
  const editLines = editedText.split('\n');

  // Build a multiset of original lines (count occurrences)
  const origCounts = new Map<string, number>();
  for (const line of origLines) {
    origCounts.set(line, (origCounts.get(line) ?? 0) + 1);
  }

  // For each edited line, check if it existed in original
  const consumed = new Map<string, number>();
  return editLines.map((line) => {
    const trimmed = line.trim();
    // Structural JSON lines (braces) are always unchanged
    if (trimmed === '{' || trimmed === '}' || trimmed === '') {
      return 'unchanged';
    }
    const available = (origCounts.get(line) ?? 0) - (consumed.get(line) ?? 0);
    if (available > 0) {
      consumed.set(line, (consumed.get(line) ?? 0) + 1);
      return 'unchanged';
    }
    // Check if a line with the same key exists in original (modified vs added)
    const keyMatch = trimmed.match(/^"([^"]+)"/);
    if (keyMatch) {
      const key = keyMatch[1];
      const hadKey = origLines.some((ol) => ol.trim().startsWith(`"${key}"`));
      return hadKey ? 'modified' : 'added';
    }
    return 'modified';
  });
}

interface ShadowTabProps {
  deviceId: string;
}

export const ShadowTab = ({ deviceId }: ShadowTabProps) => {
  const { data: shadow, isLoading, isError } = useDeviceShadow(deviceId);
  const updateDesiredMutation = useUpdateDesiredState();
  const deleteShadowMutation = useDeleteDeviceShadow();
  const [isEditing, setIsEditing] = useState(false);
  const [editValue, setEditValue] = useState('');
  const [jsonError, setJsonError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const highlightRef = useRef<HTMLPreElement>(null);

  const originalText = useMemo(
    () => (shadow ? JSON.stringify(shadow.desired, null, 2) : ''),
    [shadow]
  );

  useEffect(() => {
    if (!isEditing && shadow) {
      setEditValue(JSON.stringify(shadow.desired, null, 2));
    }
  }, [shadow, isEditing]);

  // Sync scroll between textarea and highlight layer
  const syncScroll = useCallback(() => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  const lineClassifications = useMemo(() => {
    if (!isEditing) return null;
    return classifyLines(originalText, editValue);
  }, [isEditing, originalText, editValue]);

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load device shadow. Try refreshing the page.
      </Callout>
    );
  }

  if (!shadow) {
    return <Callout icon="info-sign">No shadow data available.</Callout>;
  }

  const hasDelta = Object.keys(shadow.delta).length > 0;

  const handleEdit = () => {
    setEditValue(JSON.stringify(shadow.desired, null, 2));
    setJsonError(null);
    setIsEditing(true);
  };

  const handleCancel = () => {
    setIsEditing(false);
    setJsonError(null);
  };

  /**
   * Map cursor position from raw text to formatted text.
   * Counts non-whitespace characters before cursor in the old text,
   * then finds the same count in the new text.
   */
  const mapCursor = (oldText: string, oldPos: number, newText: string): number => {
    let semanticCount = 0;
    for (let i = 0; i < oldPos; i++) {
      const ch = oldText.charAt(i);
      if (ch && !/\s/.test(ch)) semanticCount++;
    }
    if (semanticCount === 0) return 0;
    let count = 0;
    for (let i = 0; i < newText.length; i++) {
      const ch = newText.charAt(i);
      if (ch && !/\s/.test(ch)) {
        count++;
        if (count === semanticCount) return i + 1;
      }
    }
    return newText.length;
  };

  /** Set value without auto-formatting (for Enter / raw whitespace edits) */
  const setRawValue = (value: string, cursorPos: number) => {
    setEditValue(value);
    try {
      JSON.parse(value);
      setJsonError(null);
    } catch (e) {
      setJsonError((e as SyntaxError).message);
    }
    requestAnimationFrame(() => {
      if (textareaRef.current) {
        textareaRef.current.selectionStart = textareaRef.current.selectionEnd = cursorPos;
      }
    });
  };

  const handleChange = (rawValue: string, cursorPos?: number) => {
    try {
      const parsed = JSON.parse(rawValue);
      const formatted = JSON.stringify(parsed, null, 2);
      setEditValue(formatted);
      setJsonError(null);
      // Restore cursor mapped to the formatted text
      if (cursorPos !== undefined && textareaRef.current) {
        const newPos = mapCursor(rawValue, cursorPos, formatted);
        requestAnimationFrame(() => {
          if (textareaRef.current) {
            textareaRef.current.selectionStart = textareaRef.current.selectionEnd = newPos;
          }
        });
      }
    } catch (e) {
      // Invalid JSON — keep raw text so the user can keep typing
      setEditValue(rawValue);
      setJsonError((e as SyntaxError).message);
      if (cursorPos !== undefined && textareaRef.current) {
        requestAnimationFrame(() => {
          if (textareaRef.current) {
            textareaRef.current.selectionStart = textareaRef.current.selectionEnd = cursorPos;
          }
        });
      }
    }
  };

  /** Helper: replace a range in the textarea and place the cursor */
  const spliceTextarea = (
    ta: HTMLTextAreaElement,
    start: number,
    end: number,
    insert: string,
    cursorOffset: number
  ) => {
    const before = ta.value.slice(0, start);
    const after = ta.value.slice(end);
    const next = before + insert + after;
    handleChange(next, start + cursorOffset);
  };

  const PAIRS: Record<string, string> = { '{': '}', '[': ']', '"': '"' };
  const CLOSERS = new Set(['}', ']', '"']);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    const ta = e.currentTarget;
    const { selectionStart: start, selectionEnd: end, value } = ta;

    // Tab → insert 2 spaces
    if (e.key === 'Tab') {
      e.preventDefault();
      spliceTextarea(ta, start, end, '  ', 2);
      return;
    }

    // Auto-close pairs: { [ "
    if (PAIRS[e.key] && start === end) {
      if (e.key === '"' && value[start] === '"') {
        e.preventDefault();
        handleChange(value, start + 1);
        return;
      }
      e.preventDefault();
      const pair = e.key + PAIRS[e.key];
      spliceTextarea(ta, start, end, pair, 1);
      return;
    }

    // Skip over closing bracket/brace/quote if it's already the next char
    if (CLOSERS.has(e.key) && start === end && value[start] === e.key) {
      e.preventDefault();
      handleChange(value, start + 1);
      return;
    }

    // Backspace: remove matching pair if cursor is between them
    if (e.key === 'Backspace' && start === end && start > 0) {
      const charBefore = value[start - 1];
      const charAfter = value[start];
      if (charBefore && PAIRS[charBefore] === charAfter) {
        e.preventDefault();
        spliceTextarea(ta, start - 1, start + 1, '', 0);
        return;
      }
    }

    // Enter: insert newline with auto-indent (bypass auto-format)
    if (e.key === 'Enter') {
      e.preventDefault();
      const lineStart = value.lastIndexOf('\n', start - 1) + 1;
      const currentLine = value.slice(lineStart, start);
      const indent = currentLine.match(/^(\s*)/)?.[1] ?? '';

      const charBefore = value[start - 1];
      const charAfter = value[start];

      // Between opening and closing bracket: expand with extra indent
      if (
        (charBefore === '{' && charAfter === '}') ||
        (charBefore === '[' && charAfter === ']')
      ) {
        const inner = indent + '  ';
        const insert = '\n' + inner + '\n' + indent;
        const next = value.slice(0, start) + insert + value.slice(end);
        setRawValue(next, start + inner.length + 1);
        return;
      }

      // After opening bracket: increase indent
      const extra = charBefore === '{' || charBefore === '[' ? '  ' : '';
      const insert = '\n' + indent + extra;
      const next = value.slice(0, start) + insert + value.slice(end);
      setRawValue(next, start + insert.length);
    }
  };

  const handleSave = () => {
    try {
      const parsed = JSON.parse(editValue);
      updateDesiredMutation.mutate(
        { deviceId, state: parsed },
        {
          onSuccess: () => {
            setIsEditing(false);
            setJsonError(null);
            void showSuccessToast('Desired state updated');
          },
          onError: () => void showErrorToast('Failed to update desired state'),
        }
      );
    } catch (e) {
      setJsonError((e as SyntaxError).message);
    }
  };

  return (
    <div className="shadow-tab">
      <div className="shadow-status-row">
        <Tag intent={hasDelta ? 'warning' : 'success'} minimal large>
          {hasDelta ? 'Pending' : 'In Sync'}
        </Tag>
        <span className="mono-data" style={{ fontSize: 12, opacity: 0.6 }}>
          v{shadow.version}
        </span>
      </div>

      <div className="shadow-panes-horizontal">
        <div className="shadow-pane">
          <div className="shadow-pane-header">
            <span className="section-label">Desired State</span>
            {!isEditing && (
              <Button icon="edit" minimal small onClick={handleEdit}>
                Edit
              </Button>
            )}
          </div>
          {isEditing ? (
            <>
              <div className="shadow-editor-wrapper">
                <pre
                  ref={highlightRef}
                  className="shadow-editor-highlights mono-data"
                  aria-hidden
                >
                  {editValue.split('\n').map((line, i) => {
                    const cls = lineClassifications?.[i] ?? 'unchanged';
                    return (
                      <span
                        key={i}
                        className={`editor-line editor-line--${cls}`}
                      >
                        {line || ' '}
                        {'\n'}
                      </span>
                    );
                  })}
                </pre>
                <textarea
                  ref={textareaRef}
                  className="shadow-json-editor mono-data"
                  value={editValue}
                  onChange={(e) => handleChange(e.target.value, e.target.selectionStart)}
                  onKeyDown={handleKeyDown}
                  onScroll={syncScroll}
                  spellCheck={false}
                />
              </div>
              {jsonError && (
                <div className="shadow-json-error">{jsonError}</div>
              )}
              <div className="shadow-edit-actions">
                <Button
                  intent="primary"
                  icon="floppy-disk"
                  small
                  loading={updateDesiredMutation.isPending}
                  disabled={!!jsonError}
                  onClick={handleSave}
                >
                  Save
                </Button>
                <Button small minimal onClick={handleCancel}>
                  Cancel
                </Button>
              </div>
            </>
          ) : (
            <pre className="shadow-json mono-data">
              {JSON.stringify(shadow.desired, null, 2)}
            </pre>
          )}
        </div>
        <div className="shadow-pane">
          <span className="section-label">Reported State</span>
          <pre className="shadow-json mono-data">
            {JSON.stringify(shadow.reported, null, 2)}
          </pre>
        </div>
      </div>

      {hasDelta && (
        <div className="shadow-pane shadow-delta-pane" style={{ marginTop: 12 }}>
          <span className="section-label">Delta</span>
          <pre className="shadow-diff mono-data">
            {Object.entries(shadow.delta).map(([key, desiredVal]) => {
              const reportedVal = (shadow.reported as Record<string, unknown>)[key];
              const lines: React.ReactNode[] = [];
              if (reportedVal !== undefined) {
                lines.push(
                  <span key={`${key}-old`} className="diff-line diff-removed">
                    {`- "${key}": ${JSON.stringify(reportedVal)}`}
                  </span>
                );
              }
              lines.push(
                <span key={`${key}-new`} className="diff-line diff-added">
                  {`+ "${key}": ${JSON.stringify(desiredVal)}`}
                </span>
              );
              return lines;
            })}
          </pre>
        </div>
      )}

      <Divider className="tab-divider" />

      <div className="shadow-actions tab-actions">
        <Button
          intent="danger"
          icon="trash"
          minimal
          loading={deleteShadowMutation.isPending}
          onClick={() =>
            deleteShadowMutation.mutate(deviceId, {
              onSuccess: () => void showSuccessToast('Shadow cleared'),
              onError: () => void showErrorToast('Failed to clear shadow'),
            })
          }
        >
          Clear Shadow
        </Button>
      </div>
    </div>
  );
};
