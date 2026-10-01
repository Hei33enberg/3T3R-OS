import { useEffect, useId, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from 'react';
import {
  FRAME_SLOTS,
  type FrameCenter,
  type FrameConfig,
  type FramePanel,
  type FrameSection,
  type FrameSectionSwitch,
  type FrameSide,
} from './contract';
import type { FrameLayout } from './layout';

/** Chevron. `end` points to the end of the line: right in left-to-right text, left in right-to-left text. */
function Chevron({ dir }: { dir: 'down' | 'up' | 'end' }) {
  return (
    <svg className={`f33-chevron f33-chevron--${dir}`} viewBox="0 0 24 24" aria-hidden>
      <path d="M9 6l6 6-6 6" />
    </svg>
  );
}

/** Neutral placeholder mark: a panel outline with its own half filled. Skins replace it with their glyph. */
function PanelGlyph({ side }: { side: FrameSide }) {
  return (
    <svg className="f33-glyph" viewBox="0 0 24 24" aria-hidden>
      <rect x="3.5" y="5" width="17" height="14" rx="2.5" />
      <rect className="f33-glyph-fill" x={side === 'left' ? 3.5 : 12} y="5" width="8.5" height="14" rx="2.5" />
    </svg>
  );
}

/** Placeholder for an empty slot: the slot id and what goes there. Used by the empty shell and by skins in progress. */
export function SlotBox({ slot, note, grow }: { slot: string; note?: string; grow?: boolean }) {
  const what = FRAME_SLOTS.find((s) => s.id === slot)?.what;
  return (
    <div className={grow ? 'f33-slot f33-slot-grow' : 'f33-slot'} data-slot={slot}>
      <b>{slot}</b>
      {note && <em>{note}</em>}
      {what && <span>{what}</span>}
    </div>
  );
}

/** Header of the selected section: its name, a hairline and ^ that opens the section setup. */
function SectionHead({ section, setupOpen, onSetup }: { section: FrameSection; setupOpen: boolean; onSetup: () => void }) {
  return (
    <div className="f33-head">
      <span className="f33-head-label">{section.label}</span>
      {section.count !== undefined && <span className="f33-count">{section.count}</span>}
      <span className="f33-hair" aria-hidden />
      {section.setup && (
        <button
          type="button"
          className="f33-setup"
          aria-expanded={setupOpen}
          aria-label={`${section.label} setup`}
          onClick={onSetup}
        >
          <Chevron dir={setupOpen ? 'down' : 'up'} />
        </button>
      )}
    </div>
  );
}

/** Section switch: a vertical rail (always visible) or a row of tabs that scrolls sideways. Arrow keys move along it. */
function SectionSwitch({
  kind,
  panel,
  current,
  onSelect,
}: {
  kind: FrameSectionSwitch;
  panel: FramePanel;
  current: FrameSection;
  onSelect: (id: string) => void;
}) {
  const onKey = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    const next = kind === 'rail' ? ['ArrowDown', 'ArrowUp'] : ['ArrowRight', 'ArrowLeft'];
    const step = e.key === next[0] ? 1 : e.key === next[1] ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const dir = kind === 'tabs' && document.documentElement.dir === 'rtl' ? -step : step;
    const i = panel.sections.findIndex((s) => s.id === current.id);
    const target = panel.sections[(i + dir + panel.sections.length) % panel.sections.length];
    onSelect(target.id);
    const el = e.currentTarget.querySelector<HTMLButtonElement>(`[data-id="${CSS.escape(target.id)}"]`);
    el?.focus();
  };
  return (
    <div
      className={kind === 'rail' ? 'f33-rail' : 'f33-tabs'}
      role="tablist"
      aria-orientation={kind === 'rail' ? 'vertical' : 'horizontal'}
      aria-label={`${panel.title} sections`}
      onKeyDown={onKey}
    >
      {panel.sections.map((s) => {
        const on = s.id === current.id;
        return (
          <button
            key={s.id}
            type="button"
            role="tab"
            data-id={s.id}
            aria-selected={on}
            tabIndex={on ? 0 : -1}
            className={kind === 'rail' ? 'f33-rail-item' : 'f33-tab'}
            onClick={() => onSelect(s.id)}
          >
            <span className="f33-row-label">{s.label}</span>
            {s.count !== undefined && <span className="f33-count">{s.count}</span>}
          </button>
        );
      })}
    </div>
  );
}

function PanelView({ side, panel, layout }: { side: FrameSide; panel: FramePanel; layout: FrameLayout }) {
  const [selected, setSelected] = useState(panel.sections[0]?.id);
  const [setupOpen, setSetupOpen] = useState(false);
  const current = panel.sections.find((s) => s.id === selected) ?? panel.sections[0];
  const kind: FrameSectionSwitch = panel.sectionSwitch ?? (side === 'left' ? 'rail' : 'tabs');

  const select = (id: string) => {
    setSelected(id);
    setSetupOpen(false);
  };

  const body = current && (
    <>
      {kind === 'rail' && <SectionHead section={current} setupOpen={setupOpen} onSetup={() => setSetupOpen((v) => !v)} />}
      {setupOpen && current.setup && <div className="f33-setup-body">{current.setup()}</div>}
      {current.search && <div className="f33-search">{current.search}</div>}
      <div className="f33-section" role="tabpanel" aria-label={current.label}>
        {current.render()}
      </div>
    </>
  );

  return (
    <aside className="f33-panel" data-side={side} data-switch={kind} aria-label={panel.title}>
      <header className="f33-bar">
        <button
          type="button"
          className="f33-mark"
          onClick={() => layout.toggle(side)}
          aria-label={`Close ${panel.title}`}
          aria-expanded
        >
          {panel.mark ?? <PanelGlyph side={side} />}
        </button>
        <span className="f33-title">{panel.title}</span>
        {panel.count !== undefined && <span className="f33-count">{panel.count}</span>}
      </header>

      {kind === 'rail' ? (
        <div className="f33-cols">
          {current && <SectionSwitch kind="rail" panel={panel} current={current} onSelect={select} />}
          <div className="f33-col">
            {body}
            {panel.bottom && <div className="f33-bottom">{panel.bottom}</div>}
          </div>
        </div>
      ) : (
        <>
          {panel.top && <div className="f33-panel-top">{panel.top}</div>}
          {current && <SectionSwitch kind="tabs" panel={panel} current={current} onSelect={select} />}
          {body}
          {panel.bottom && <div className="f33-bottom">{panel.bottom}</div>}
        </>
      )}
    </aside>
  );
}

function ViewPill({ center }: { center: FrameCenter }) {
  const views = center.views ?? [];
  const active = center.activeView ?? views[0]?.id;
  if (views.length === 0) return null;
  return (
    <div className="f33-pill" role="radiogroup" aria-label="View">
      {views.map((v) => (
        <button
          key={v.id}
          type="button"
          role="radio"
          aria-checked={v.id === active}
          className="f33-pill-option"
          onClick={() => center.onViewChange?.(v.id)}
        >
          {v.icon}
          <span>{v.label}</span>
        </button>
      ))}
    </div>
  );
}

function FloatingMenu({ center, initialOpen }: { center: FrameCenter; initialOpen?: boolean }) {
  const [open, setOpen] = useState(!!initialOpen);
  const sheetId = useId();

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open]);

  return (
    <>
      {open && <div className="f33-menu-catch" onClick={() => setOpen(false)} aria-hidden />}
      <div className="f33-menu">
        {open && (
          <div className="f33-menu-sheet" id={sheetId} role="menu">
            {center.menu.map((item) => (
              <div key={item.id}>
                {item.startsGroup && <div className="f33-menu-hair" aria-hidden />}
                <button
                  type="button"
                  role="menuitem"
                  className="f33-row f33-menu-item"
                  onClick={() => {
                    item.onSelect?.();
                    setOpen(false);
                  }}
                >
                  <span className="f33-menu-icon" aria-hidden>{item.icon}</span>
                  <span className="f33-row-label">{item.label}</span>
                  {item.opensScreen && <Chevron dir="end" />}
                </button>
              </div>
            ))}
          </div>
        )}
        <button
          type="button"
          className="f33-menu-handle"
          aria-expanded={open}
          aria-controls={sheetId}
          onClick={() => setOpen((v) => !v)}
        >
          <svg viewBox="0 0 14 6" aria-hidden>
            <path d="M0 0.75h14M0 5.25h14" />
          </svg>
          <span>{center.menuLabel ?? 'Menu'}</span>
        </button>
      </div>
    </>
  );
}

export interface FrameShellProps {
  config: FrameConfig;
  layout: FrameLayout;
  /** Start with the floating menu open (for previews and screenshots). */
  initialMenuOpen?: boolean;
  /** Extra layer over the center, e.g. a slot label while a skin is in progress. */
  centerOverlay?: ReactNode;
}

/** The empty 33.0S frame: left panel, live center, right panel. Layout and behaviour only; the skin brings everything else. */
export function FrameShell({ config, layout, initialMenuOpen, centerOverlay }: FrameShellProps) {
  const { open, isLaptop } = layout;
  return (
    <div className="f33-frame" data-laptop={isLaptop} data-left={open.left} data-right={open.right}>
      {open.left && <PanelView side="left" panel={config.left} layout={layout} />}

      <main className="f33-center" aria-label="Center">
        <div className="f33-view">{config.center.view}</div>
        {centerOverlay}
        <div className="f33-top">
          <div className="f33-top-side">
            {!open.left && (
              <button type="button" className="f33-mark" onClick={() => layout.toggle('left')} aria-label={`Open ${config.left.title}`}>
                {config.left.mark ?? <PanelGlyph side="left" />}
              </button>
            )}
          </div>
          <ViewPill center={config.center} />
          <div className="f33-top-side f33-top-right">
            {!open.right && (
              <button type="button" className="f33-mark" onClick={() => layout.toggle('right')} aria-label={`Open ${config.right.title}`}>
                {config.right.mark ?? <PanelGlyph side="right" />}
              </button>
            )}
          </div>
        </div>
        <FloatingMenu center={config.center} initialOpen={initialMenuOpen} />
      </main>

      {open.right && <PanelView side="right" panel={config.right} layout={layout} />}
    </div>
  );
}
